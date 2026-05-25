// Where: src/main.rs
// What: Binary entrypoint for the read-only kinic-context CLI.
// Why: Keep the executable thin and delegate all behavior to the library core.
use anyhow::Result;
use clap::Parser;
use kinic_context_cli::{
    catalog::WikiCliSourceCatalog,
    cli::{Cli, Command},
    config::ReadConfig,
    engine::ContextEngine,
    output::render_json,
    provider::WikiCliSourceQueryProvider,
};
use kinic_context_core::types::FilterSourcesArgs;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let output = match cli.command {
        Command::Cite(args) => ContextEngine::citer().cite(&args.pack)?,
        other => {
            let engine = load_engine().await?;
            match other {
                Command::Resolve(args) => engine.resolve(&args.query, args.max_sources).await?,
                Command::Query(args) => {
                    engine
                        .query(
                            &args.source_id,
                            &args.query,
                            args.version.as_deref(),
                            args.top_k,
                        )
                        .await?
                }
                Command::Pack(args) => {
                    engine
                        .pack(&args.query, args.max_sources, args.max_tokens)
                        .await?
                }
                Command::ListSources(_) => engine.list_sources().await?,
                Command::FilterSources(args) => {
                    engine
                        .filter_sources(FilterSourcesArgs {
                            domain: args.domain,
                            trust: args.trust,
                            version: args.version,
                            limit: args.limit,
                        })
                        .await?
                }
                Command::Cite(_) => unreachable!("handled above"),
            }
        }
    };

    println!("{}", render_json(&output, cli.pretty)?);
    Ok(())
}

async fn load_engine() -> Result<ContextEngine<WikiCliSourceCatalog, WikiCliSourceQueryProvider>> {
    let config = ReadConfig::from_env()?;
    let catalog =
        WikiCliSourceCatalog::new(config.wiki_cli_bin.clone(), config.database_id.clone());
    let provider = WikiCliSourceQueryProvider::new(config.wiki_cli_bin, config.database_id);
    Ok(ContextEngine::new(catalog, provider))
}
