// Where: src/main.rs
// What: Binary entrypoint for the read-only kinic-context CLI.
// Why: Keep the executable thin and delegate all behavior to the library core.
use anyhow::Result;
use clap::Parser;
use kinic_context_cli::{
    catalog::IcSourceCatalog,
    cli::{Cli, Command},
    config::ReadConfig,
    engine::ContextEngine,
    output::render_json,
    provider::IcSourceQueryProvider,
};
use kinic_context_core::client::QueryClient;
use kinic_context_core::types::FilterSourcesArgs;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let output = match cli.command {
        Command::Cite(args) => ContextEngine::citer().cite(&args.pack)?,
        other => {
            let engine = load_engine().await?;
            match other {
                Command::Resolve(args) => {
                    engine
                        .resolve(&args.query, args.max_sources, args.include_skills)
                        .await?
                }
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
                        .pack(
                            &args.query,
                            args.max_sources,
                            args.max_tokens,
                            args.include_skills,
                        )
                        .await?
                }
                Command::ListSources(args) => engine.list_sources(args.include_skills).await?,
                Command::FilterSources(args) => {
                    let include_skills = args.include_skills;
                    engine
                        .filter_sources(FilterSourcesArgs {
                            domain: args.domain,
                            trust: args.trust,
                            version: args.version,
                            limit: args.limit,
                        }, include_skills)
                        .await?
                }
                Command::Cite(_) => unreachable!("handled above"),
            }
        }
    };

    println!("{}", render_json(&output, cli.pretty)?);
    Ok(())
}

async fn load_engine() -> Result<ContextEngine<IcSourceCatalog, IcSourceQueryProvider>> {
    let config = ReadConfig::from_env()?;
    let client = QueryClient::new(&config.ic_host, config.fetch_root_key).await?;
    let catalog = IcSourceCatalog::new(client.clone(), config.catalog_canister_id);
    let provider = IcSourceQueryProvider::new(client);
    Ok(ContextEngine::new(catalog, provider))
}
