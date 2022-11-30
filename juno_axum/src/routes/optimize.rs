use axum::{extract::State, http::StatusCode, response::IntoResponse, routing, Json, Router};
use itertools::Itertools;
use juno::{
    clients::juno_core,
    genetics::{
        crossover, mutation, reinsertion, selection, Chromosome, GeneticAlgorithm, Individual,
    },
    statistics::Statistics,
    trading::{
        trade, BasicEvaluation, BasicEvaluationInput, EvaluationAggregation, EvaluationStatistic,
        TradeInput, TradingParams, TradingParamsContext,
    },
    Candle, ExchangeInfo, Interval, SymbolExt, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tracing::info;

use crate::error::Error;

#[derive(Deserialize)]
struct Params {
    population_size: usize,
    generations: usize,
    hall_of_fame_size: usize,
    seed: Option<u64>,

    exchange: String,
    start: Timestamp,
    end: Timestamp,
    quote: f64,
    training_symbols: Vec<String>,

    validation_symbols: Vec<String>,

    evaluation_statistic: EvaluationStatistic,
    evaluation_aggregation: EvaluationAggregation,

    context: TradingParamsContext,
}

impl Params {
    fn iter_symbols(&self) -> impl Iterator<Item = &String> {
        self.training_symbols.iter().chain(&self.validation_symbols)
    }
}

#[derive(Serialize)]
struct GenerationOutput {
    // We need to store generation number because we are filtering out generations with no change
    // in top.
    nr: usize,
    hall_of_fame: Vec<IndividualStats>,
}

#[derive(Serialize)]
struct IndividualStats {
    individual: Individual<TradingParams>,
    symbol_stats: HashMap<String, Statistics>,
}

#[derive(Serialize)]
struct OptimizeResult {
    generations: Vec<GenerationOutput>,
    seed: u64,
}

#[derive(Serialize)]
struct Info {
    evaluation_statistics: [EvaluationStatistic; 4],
    evaluation_aggregations: [EvaluationAggregation; 3],
}

pub fn routes() -> Router<Arc<juno_core::Client>> {
    Router::new()
        .route("/", routing::get(get))
        .route("/", routing::post(post))
}

async fn get() -> impl IntoResponse {
    Json(Info {
        evaluation_statistics: EvaluationStatistic::values(),
        evaluation_aggregations: EvaluationAggregation::values(),
    })
}

async fn post(
    State(juno_core_client): State<Arc<juno_core::Client>>,
    Json(args): Json<Params>,
) -> Result<impl IntoResponse, Error> {
    // Gather data.
    info!("gathering data");
    let symbols: Vec<_> = args.iter_symbols().unique().cloned().collect();
    let (exchange_info, candles, prices) = crate::exchange::gather_exchange_info_candles_prices(
        &juno_core_client,
        &args.exchange,
        &symbols,
        &args.context.trader.intervals,
        args.start,
        args.end,
    )
    .await?;

    // Optimize in parallel.
    info!("optimizing");
    let optimize_result =
        tokio_rayon::spawn(move || par_optimize(&args, &exchange_info, &candles, &prices)).await;

    Ok((StatusCode::OK, Json(optimize_result)))
}

fn par_optimize(
    args: &Params,
    exchange_info: &ExchangeInfo,
    candles: &HashMap<String, HashMap<Interval, Vec<Candle>>>,
    prices: &HashMap<String, Vec<f64>>,
) -> OptimizeResult {
    // Optimize.
    let algo = GeneticAlgorithm::new(
        BasicEvaluation::new(&BasicEvaluationInput {
            exchange_info,
            candles,
            prices,
            symbols: &args.training_symbols,
            intervals: &args.context.trader.intervals,
            start: args.start,
            end: args.end,
            quote: args.quote,
            evaluation_statistic: args.evaluation_statistic,
            evaluation_aggregation: args.evaluation_aggregation,
        }),
        selection::EliteSelection { shuffle: false },
        // selection::TournamentSelection::default(),
        // selection::GenerateRandomSelection {}, // For random search.
        crossover::UniformCrossover::new(0.5),
        mutation::UniformMutation::new(0.25),
        reinsertion::EliteReinsertion::new(0.75, 0.5),
        // reinsertion::PureReinsertion {}, // For random search.
    );
    let pop_size = args.population_size;
    let generations = args.generations;
    let hall_of_fame_size = args.hall_of_fame_size;
    let seed = args.seed;
    let context = args.context.clone();
    let evolution = algo.evolve(
        pop_size,
        generations,
        hall_of_fame_size,
        seed,
        on_generation,
        &context,
    );

    // Evaluate hall of fame.
    let stats_interval = Interval::DAY_MS;
    let mut best_fitnesses = vec![f64::NAN; args.hall_of_fame_size];
    let gen_stats = evolution
        .generations
        .into_iter()
        .filter(|gen| {
            let mut pass = false;
            for (best_ind, best_fitness) in gen.hall_of_fame.iter().zip(best_fitnesses.iter_mut()) {
                if best_fitness.is_nan() || best_ind.fitness > *best_fitness {
                    *best_fitness = best_ind.fitness;
                    pass = true;
                }
            }
            pass
        })
        .enumerate()
        .map(|(nr, gen)| {
            let hall_of_fame: Vec<_> = gen
                .hall_of_fame
                .into_iter()
                .map(|ind| {
                    let symbol_stats: HashMap<_, _> = args
                        .iter_symbols()
                        .unique()
                        .map(|symbol| {
                            let summary = trade(
                                &ind.chromosome,
                                &TradeInput {
                                    candles: &candles[symbol][&ind.chromosome.trader.interval],
                                    fees: &exchange_info.fees[symbol],
                                    filters: &exchange_info.filters[symbol],
                                    borrow_info: &exchange_info.borrow_info[symbol]
                                        [symbol.base_asset()],
                                    margin_multiplier: 2,
                                    quote: args.quote,
                                    long: true,
                                    short: true,
                                },
                            );
                            let stats =
                                Statistics::compose(&summary, symbol, prices, stats_interval);
                            (symbol.clone(), stats)
                        })
                        .collect();
                    IndividualStats {
                        symbol_stats,
                        individual: ind,
                    }
                })
                .collect();
            GenerationOutput { nr, hall_of_fame }
        })
        .collect();

    OptimizeResult {
        generations: gen_stats,
        seed: evolution.seed,
    }
}

fn on_generation<T: Chromosome>(nr: usize, gen: &juno::genetics::Generation<T>) {
    println!("gen {} best fitness {}", nr, gen.hall_of_fame[0].fitness);
    println!("{:?}", gen.timings);
}
