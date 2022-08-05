use anyhow::Result;
use axum::{http::StatusCode, response::IntoResponse, routing, Extension, Json, Router};
use futures::future::{try_join, try_join_all};
use juno::{
    clients::juno_core,
    genetics::{
        crossover, mutation, reinsertion, selection, Chromosome, Evolution, Generation,
        GeneticAlgorithm, Individual,
    },
    statistics::Statistics,
    trading::{
        trade, BasicEvaluation, EvaluationAggregation, EvaluationStatistic, TradingParams,
        TradingParamsContext, TradingSummary,
    },
    CandleType, Interval, SymbolExt, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

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
struct EvolutionStats {
    generations: Vec<GenerationOutput>,
    seed: u64,
}

#[derive(Serialize)]
struct Info {
    evaluation_statistics: [EvaluationStatistic; 4],
    evaluation_aggregations: [EvaluationAggregation; 3],
}

pub fn routes() -> Router {
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
    Json(args): Json<Params>,
    Extension(juno_core_client): Extension<Arc<juno_core::Client>>,
) -> Result<impl IntoResponse, StatusCode> {
    let evolution = optimize(&juno_core_client, &args)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut best_fitnesses = vec![f64::NAN; args.hall_of_fame_size];
    let gen_stats_tasks = evolution
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
        .map(|(i, gen)| backtest_generation(&juno_core_client, &args, i, gen));

    let gen_stats = try_join_all(gen_stats_tasks)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::OK,
        Json(EvolutionStats {
            generations: gen_stats,
            seed: evolution.seed,
        }),
    ))
}

async fn backtest_generation(
    juno_core_client: &juno_core::Client,
    args: &Params,
    nr: usize,
    gen: Generation<TradingParams>,
) -> Result<GenerationOutput> {
    let hall_of_fame_tasks = gen.hall_of_fame.into_iter().map(|ind| async {
        let symbol_stats = try_join_all(args.iter_symbols().map(|symbol| (symbol, &ind)).map(
            |(symbol, ind)| async {
                let summary = backtest(juno_core_client, args, symbol, &ind.chromosome).await?;
                let stats = get_stats(juno_core_client, args, symbol, &summary).await?;
                Ok::<_, anyhow::Error>((symbol.clone(), stats))
            },
        ))
        .await?
        .into_iter()
        .collect();

        Ok::<_, anyhow::Error>(IndividualStats {
            individual: ind,
            symbol_stats,
        })
    });
    let hall_of_fame = try_join_all(hall_of_fame_tasks).await?;

    Ok::<_, anyhow::Error>(GenerationOutput { nr, hall_of_fame })
}

async fn optimize(
    juno_core_client: &juno_core::Client,
    args: &Params,
) -> Result<Evolution<TradingParams>> {
    let algo = GeneticAlgorithm::new(
        BasicEvaluation::new(
            juno_core_client,
            &args.exchange,
            &args.training_symbols,
            &args.context.trader.intervals,
            args.start,
            args.end,
            args.quote,
            args.evaluation_statistic,
            args.evaluation_aggregation,
        )
        .await?,
        selection::EliteSelection { shuffle: false },
        // selection::TournamentSelection::default(),
        // selection::GenerateRandomSelection {}, // For random search.
        crossover::UniformCrossover::new(0.5),
        mutation::UniformMutation::new(0.25),
        reinsertion::EliteReinsertion::new(0.75, 0.5),
        // reinsertion::PureReinsertion {}, // For random search.
    );
    let evolution = algo.evolve(
        args.population_size,
        args.generations,
        args.hall_of_fame_size,
        args.seed,
        on_generation,
        &args.context,
    );
    Ok(evolution)
}

fn on_generation<T: Chromosome>(nr: usize, gen: &juno::genetics::Generation<T>) {
    println!("gen {} best fitness {}", nr, gen.hall_of_fame[0].fitness);
    println!("{:?}", gen.timings);
}

async fn backtest(
    juno_core_client: &juno_core::Client,
    args: &Params,
    symbol: &str,
    chromosome: &TradingParams,
) -> Result<TradingSummary> {
    let exchange_info_task = juno_core_client.get_exchange_info(&args.exchange);
    let candles_task = juno_core_client.list_candles(
        &args.exchange,
        symbol,
        chromosome.trader.interval,
        args.start,
        args.end,
        CandleType::Regular,
    );

    let (exchange_info, candles) = try_join(exchange_info_task, candles_task).await?;

    Ok(trade(
        chromosome,
        &candles,
        &exchange_info.fees[symbol],
        &exchange_info.filters[symbol],
        &exchange_info.borrow_info[symbol][symbol.base_asset()],
        2,
        args.quote,
        true,
        true,
    ))
}

async fn get_stats(
    juno_core_client: &juno_core::Client,
    args: &Params,
    symbol: &str,
    summary: &TradingSummary,
) -> Result<Statistics> {
    let stats_interval = Interval::DAY_MS;

    let mut assets = vec![symbol.base_asset(), symbol.quote_asset()];
    // Benchmark asset for extended statistics.
    assets.push("btc");
    let prices = juno_core_client
        .map_asset_prices(
            &args.exchange,
            &assets,
            stats_interval,
            args.start,
            args.end,
            "usdt",
        )
        .await?;

    let stats = Statistics::compose(summary, symbol, &prices, stats_interval);

    Ok(stats)
}
