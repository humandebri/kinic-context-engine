// Where: src/cli.rs
// What: Public command-line interface for the read-only context runtime.
// Why: Restrict the surfaced behavior to safe retrieval commands only.
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "kinic-context",
    version,
    about = "Read-only CLI for public context retrieval and evidence packs"
)]
pub struct Cli {
    #[arg(long, help = "Pretty-print JSON output for humans")]
    pub pretty: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Resolve a natural language query into candidate public sources")]
    Resolve(ResolveArgs),
    #[command(about = "Query a single public source and return ranked snippets")]
    Query(QueryArgs),
    #[command(about = "Build an evidence pack by resolving and querying multiple sources")]
    Pack(PackArgs),
    #[command(about = "Extract citation and provenance details from an evidence pack")]
    Cite(CiteArgs),
    #[command(about = "List all sources exposed by the catalog canister")]
    ListSources(ListSourcesArgs),
    #[command(about = "Filter catalog sources by metadata")]
    FilterSources(FilterSourcesArgs),
}

#[derive(Debug, Args)]
pub struct ResolveArgs {
    #[arg(help = "Natural language query to resolve")]
    pub query: String,

    #[arg(
        long,
        default_value_t = 5,
        help = "Maximum number of sources to return"
    )]
    pub max_sources: usize,

    #[arg(long, help = "Include skill knowledge sources in resolution")]
    pub include_skills: bool,
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    #[arg(help = "Stable source identifier, such as /vercel/next.js")]
    pub source_id: String,

    #[arg(help = "Natural language query for the selected source")]
    pub query: String,

    #[arg(long, help = "Optional source version filter")]
    pub version: Option<String>,

    #[arg(
        long,
        default_value_t = 5,
        help = "Maximum number of snippets to return"
    )]
    pub top_k: usize,
}

#[derive(Debug, Args)]
pub struct PackArgs {
    #[arg(help = "Natural language query to resolve and pack")]
    pub query: String,

    #[arg(
        long,
        default_value_t = 5,
        help = "Maximum number of sources to fan out to"
    )]
    pub max_sources: usize,

    #[arg(
        long,
        default_value_t = 3000,
        help = "Requested token budget for the pack"
    )]
    pub max_tokens: usize,

    #[arg(long, help = "Include skill knowledge sources in resolution")]
    pub include_skills: bool,
}

#[derive(Debug, Args)]
pub struct CiteArgs {
    #[arg(help = "Inline evidence pack JSON")]
    pub pack: String,
}

#[derive(Debug, Args)]
pub struct ListSourcesArgs {
    #[arg(long, help = "Include skill knowledge sources")]
    pub include_skills: bool,
}

#[derive(Debug, Args)]
pub struct FilterSourcesArgs {
    #[arg(long, help = "Optional domain filter; `skill_knowledge` can be queried directly")]
    pub domain: Option<String>,

    #[arg(long, help = "Optional trust filter")]
    pub trust: Option<String>,

    #[arg(long, help = "Optional supported version filter")]
    pub version: Option<String>,

    #[arg(long, help = "Optional result limit")]
    pub limit: Option<u32>,

    #[arg(long, help = "Include skill knowledge sources in the result set")]
    pub include_skills: bool,
}
