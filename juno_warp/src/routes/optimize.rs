use super::custom_reject;
use anyhow::{Error, Result};
use futures::{
    future::{try_join, try_join_all},
    TryFutureExt,
};
use juno::{
    candles,
    genetics::{
        crossover, mutation, reinsertion, selection, Chromosome, Evolution, GeneticAlgorithm,
        Individual,
    },
    statistics::Statistics,
    storage,
    trading::{
        trade, BasicEvaluation, EvaluationAggregation, EvaluationStatistic, TradingParams,
        TradingParamsContext, TradingSummary,
    },
    Interval, SymbolExt, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use warp::{reply, Filter, Rejection, Reply};

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
struct Generation<'a> {
    // We need to store generation number because we are filtering out generations with no change
    // in top.
    nr: usize,
    hall_of_fame: Vec<IndividualStats<'a>>,
}

#[derive(Serialize)]
struct IndividualStats<'a> {
    individual: Individual<TradingParams>,
    symbol_stats: HashMap<&'a str, Statistics>,
}

#[derive(Serialize)]
struct EvolutionStats<'a> {
    generations: Vec<Generation<'a>>,
    seed: u64,
}

#[derive(Serialize)]
struct Info {
    evaluation_statistics: [EvaluationStatistic; 4],
    evaluation_aggregations: [EvaluationAggregation; 3],
}

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("optimize").and(get().or(post()))
}

fn get() -> impl Filter<Extract = (reply::Json,), Error = Rejection> + Clone {
    warp::get().map(|| {
        reply::json(&Info {
            evaluation_statistics: EvaluationStatistic::values(),
            evaluation_aggregations: EvaluationAggregation::values(),
        })
    })
}

fn post() -> impl Filter<Extract = (reply::Json,), Error = Rejection> + Clone {
    warp::post()
        .and(warp::body::json())
        .and_then(|args: Params| async move {
            process(args).await.map_err(custom_reject) // TODO: return 400
        })
}

async fn process(args: Params) -> Result<reply::Json> {
    let evolution = optimize(&args).await?;
    let mut best_fitnesses = vec![f64::NAN; args.hall_of_fame_size];
    let gen_stats_tasks = evolution
        .generations
        .into_iter()
        .enumerate()
        .filter(|(_, gen)| {
            let mut pass = false;
            for (best_ind, best_fitness) in gen.hall_of_fame.iter().zip(best_fitnesses.iter_mut()) {
                if best_fitness.is_nan() || best_ind.fitness > *best_fitness {
                    *best_fitness = best_ind.fitness;
                    pass = true;
                }
            }
            pass
        })
        .map(|(i, gen)| (i, gen, &args))
        .map(|(i, gen, args)| async move {
            let hall_of_fame_tasks = gen.hall_of_fame.into_iter().map(|ind| async move {
                let symbol_stats =
                    try_join_all(args.iter_symbols().map(|symbol| (symbol, &ind)).map(
                        |(symbol, ind)| async move {
                            let summary = backtest(args, symbol, &ind.chromosome).await?;
                            let stats = get_stats(args, symbol, &summary).await?;
                            Ok::<_, Error>((symbol.as_ref(), stats))
                        },
                    ))
                    .await?
                    .into_iter()
                    .collect();

                Ok::<_, Error>(IndividualStats {
                    individual: ind,
                    symbol_stats,
                })
            });
            let hall_of_fame = try_join_all(hall_of_fame_tasks).await?;

            Ok::<_, Error>(Generation {
                nr: i,
                hall_of_fame,
            })
        });

    let gen_stats = try_join_all(gen_stats_tasks).await?;

    Ok(reply::json(&EvolutionStats {
        generations: gen_stats,
        seed: evolution.seed,
    }))
}

async fn optimize(args: &Params) -> Result<Evolution<TradingParams>> {
    let algo = GeneticAlgorithm::new(
        BasicEvaluation::new(
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
    args: &Params,
    symbol: &str,
    chromosome: &TradingParams,
) -> Result<TradingSummary> {
    let exchange_info_task = storage::get_exchange_info(&args.exchange).map_err(Error::from);
    let candles_task = candles::list_candles(
        &args.exchange,
        symbol,
        chromosome.trader.interval,
        args.start,
        args.end,
        false,
    )
    .map_err(Error::from);

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

async fn get_stats(args: &Params, symbol: &str, summary: &TradingSummary) -> Result<Statistics> {
    let stats_interval = Interval::DAY_MS;

    // Stats base.
    let stats_candles_task = candles::list_candles(
        &args.exchange,
        symbol,
        stats_interval,
        args.start,
        args.end,
        true,
    );

    // Stats quote (optional).
    let stats_fiat_candles_task = candles::list_candles(
        "binance",
        "btc-usdt",
        stats_interval,
        args.start,
        args.end,
        true,
    );

    let (stats_candles, stats_fiat_candles) =
        try_join(stats_candles_task, stats_fiat_candles_task).await?;

    // let stats_quote_prices = None;
    let stats_quote_prices = Some(candles::candles_to_prices(&stats_fiat_candles, None));
    let stats_base_prices =
        candles::candles_to_prices(&stats_candles, stats_quote_prices.as_deref());

    let stats = Statistics::compose(
        summary,
        &stats_base_prices,
        stats_quote_prices.as_deref(),
        stats_interval,
    );

    Ok(stats)
}
