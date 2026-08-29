//! Torimemo's command line.
//!
//! Everything here is a thin shell over `torimemo-core`. The CLI is the
//! operator's surface — backfill, inspect, search — while day-to-day capture
//! happens through the bot and the extension.

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use torimemo_core::{Error, Result, Source, Store, import};
use torimemo_embed::{Embedder, Provider, backfill, rank_by_similarity};

#[derive(Parser, Debug)]
#[command(name = "torimemo", version, about = "Bookmark capture and recall")]
struct Cli {
    /// Path to the store. Defaults to `~/.torimemo/torimemo.db`.
    ///
    /// Reads `TORIMEMO_DB` when the flag is absent, so a container can set the
    /// path once in its environment rather than repeating it on every command.
    #[arg(long, global = true, env = "TORIMEMO_DB")]
    db: Option<PathBuf>,

    /// Embedding model. `deterministic` needs no download and no network,
    /// but only captures lexical overlap.
    #[arg(long, global = true, default_value = "bge-small-en-v1.5")]
    model: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum TokenAction {
    /// Mint a token. The secret is printed once and never stored.
    Issue {
        /// Operator label, e.g. `odin`.
        #[arg(long)]
        name: String,
        /// `read` or `read-write`. Read-only unless asked otherwise, and
        /// fixed for the token's life.
        #[arg(long, default_value = "read")]
        scope: String,
    },
    /// List every token, including revoked ones.
    List,
    /// Revoke a token by id.
    Revoke {
        /// The id from `token list`.
        id: String,
    },
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Import every URL found in a directory of markdown files.
    ImportVault {
        /// Directory to walk, non-recursively.
        path: PathBuf,
    },
    /// Import a browser's Netscape-format bookmark export.
    ImportBrowser {
        /// The exported HTML file.
        path: PathBuf,
    },
    /// Import a GitHub user's starred repositories.
    ///
    /// Stars are maintained where you press the button, so re-running this
    /// picks up everything starred since last time.
    ImportStars {
        /// Whose stars to import.
        user: String,
        /// A personal access token. Unauthenticated requests are limited to
        /// 60 per hour; a token raises that to 5000.
        #[arg(long, env = "GITHUB_TOKEN")]
        token: Option<String>,
        /// Include repositories the owner has archived.
        #[arg(long)]
        include_archived: bool,
        /// Include forks.
        #[arg(long)]
        include_forks: bool,
    },
    /// Add a single link.
    Add {
        /// The URL to capture.
        url: String,
        /// Text to record alongside it.
        #[arg(long)]
        context: Option<String>,
    },
    /// Fetch page titles and descriptions for un-enriched bookmarks.
    Enrich {
        /// Requests in flight at once.
        #[arg(long, default_value_t = 16)]
        concurrency: usize,
        /// Milliseconds to wait between requests to the same host.
        #[arg(long, default_value_t = 500)]
        host_delay: u64,
        /// Per-request timeout in seconds.
        #[arg(long, default_value_t = 10)]
        timeout: u64,
        /// Stop after this many bookmarks.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Assign tags with a labelling model.
    Label {
        /// Labeller: `rules` needs no API key; anything else is an Anthropic
        /// model id and requires `ANTHROPIC_API_KEY`.
        #[arg(long, default_value = "claude-haiku-4-5")]
        labeller: String,
        /// Stop after this many bookmarks.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Write the labelled corpus as JSON Lines, for training a local model.
    ExportTraining {
        /// Which labeller's output to export.
        #[arg(long, default_value = "claude-haiku-4-5")]
        labeller: String,
        /// Where to write. Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Show tag counts across the corpus.
    Tags,
    /// Review and remove bookmarks that are no longer worth keeping.
    ///
    /// Reports what it would remove and why. Nothing is deleted without
    /// `--apply`.
    Prune {
        /// Categories to consider. Repeat the flag or use `all`. Omitted, only
        /// the objective ones run: dead, walled, untitled, asset, session.
        #[arg(long = "reason", value_name = "CATEGORY")]
        reasons: Vec<String>,
        /// Actually delete. Without this the command only reports.
        #[arg(long)]
        apply: bool,
        /// Print every candidate rather than a few per category.
        #[arg(long)]
        verbose: bool,
    },
    /// Train the local classifier on the teacher's labels.
    Train {
        /// Which labeller's output to learn from.
        #[arg(long, default_value = "claude-haiku-4-5")]
        teacher: String,
        /// Where to write the trained model.
        #[arg(long, default_value = "classifier.json")]
        out: PathBuf,
        /// A label needs this many positive examples to be fitted.
        #[arg(long, default_value_t = 20)]
        min_support: usize,
    },
    /// Predict tags for a URL with the trained local classifier.
    Suggest {
        /// The URL, which must already be in the store and embedded.
        url: String,
        /// Path to the trained model.
        #[arg(long, default_value = "classifier.json")]
        model_path: PathBuf,
        /// Minimum probability to report.
        #[arg(long, default_value_t = 0.5)]
        threshold: f32,
    },
    /// Embed every bookmark that has no vector for the current model.
    Embed {
        /// How many to embed per batch.
        #[arg(long, default_value_t = 64)]
        batch: usize,
    },
    /// Semantic search over embeddings.
    Recall {
        /// What to look for, in your own words.
        query: String,
        /// Maximum results.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Minimum cosine similarity.
        #[arg(long, default_value_t = 0.0)]
        floor: f32,
    },
    /// Lexical search over URL, title, and description.
    Search {
        /// The query.
        query: String,
        /// Maximum results.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Manage the bearer tokens an agent authenticates with.
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
    /// Serve the HTTP API on loopback.
    Serve {
        /// Port to bind.
        #[arg(long, default_value_t = 7645)]
        port: u16,
    },
    /// Corpus counts.
    Stats,
    /// Links captured more than once, most-captured first.
    Repeats {
        /// Maximum results.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            let mut source = std::error::Error::source(&error);
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<()> {
    let path = database_path(cli.db.as_deref())?;
    let mut store = Store::open(&path)?;

    match &cli.command {
        Command::ImportVault { path } => import_vault(&mut store, path),
        Command::ImportBrowser { path } => import_browser(&mut store, path),
        Command::ImportStars { user, token, include_archived, include_forks } => {
            let config = torimemo_enrich::stars::Config {
                user: user.clone(),
                token: token.clone(),
                skip_archived: !include_archived,
                skip_forks: !include_forks,
            };

            let runtime = tokio::runtime::Runtime::new()?;
            let (captures, mut summary) =
                runtime.block_on(torimemo_enrich::stars::fetch(&config))?;

            println!("{} stars, {} skipped", summary.fetched, summary.skipped);

            let outcome = store.ingest_batch(&captures)?;
            summary.created = outcome.created;
            summary.merged = outcome.merged;

            // GitHub already told us what each repository is, so seed the
            // bookmark's title and description from the star rather than
            // making the enrichment pass fetch a page to learn the same thing.
            // Only where nothing is set: a title the user or a fetch produced
            // is better evidence than the API's summary.
            for capture in &captures {
                let Some(context) = &capture.context else { continue };
                let Some(bookmark) = store.bookmark_by_url(&capture.raw_url)? else { continue };
                if bookmark.title.is_some() {
                    continue;
                }
                let (name, description) = context
                    .split_once(" — ")
                    .map_or((context.as_str(), None), |(name, rest)| (name, Some(rest)));
                store.set_metadata(bookmark.id, Some(name), description)?;
            }

            println!(
                "imported {} new, {} already known, {} unreadable",
                summary.created,
                summary.merged,
                outcome.skipped.len()
            );
            Ok(())
        }
        Command::Add { url, context } => {
            let mut capture = torimemo_core::NewCapture::new(url.clone(), Source::Api);
            if let Some(context) = context {
                capture = capture.with_context(context.clone());
            }
            let ingested = store.ingest(&capture)?;
            let bookmark = store.bookmark(ingested.bookmark_id())?;
            let label = if ingested.is_new() { "added" } else { "merged" };
            if let Some(bookmark) = bookmark {
                println!(
                    "{label}: {} (captured {}x)",
                    bookmark.canonical_url, bookmark.capture_count
                );
            }
            Ok(())
        }
        Command::Enrich { concurrency, host_delay, timeout, limit } => {
            let config = torimemo_enrich::Config {
                concurrency: *concurrency,
                per_host_delay: std::time::Duration::from_millis(*host_delay),
                timeout: std::time::Duration::from_secs(*timeout),
                max_attempts: 3,
                limit: limit.unwrap_or(usize::MAX),
            };
            let runtime = tokio::runtime::Runtime::new()?;
            let summary = runtime.block_on(torimemo_enrich::run(&store, &config, |summary| {
                progress_line(
                    summary.total(),
                    100,
                    &format!(
                        "{} enriched, {} bare, {} dead, {} failed",
                        summary.enriched, summary.no_metadata, summary.dead, summary.failed
                    ),
                );
            }))?;
            println!(
                "\rdone: {} enriched, {} bare, {} dead, {} failed ({} total)",
                summary.enriched,
                summary.no_metadata,
                summary.dead,
                summary.failed,
                summary.total()
            );
            Ok(())
        }
        Command::Label { labeller, limit } => {
            let limit = limit.unwrap_or(usize::MAX);
            let summary = if labeller == "rules" {
                let engine = torimemo_enrich::RuleBased;
                println!("labelling with {}", torimemo_enrich::Labeller::model(&engine));
                run_labelling(&mut store, &engine, limit)
            } else {
                let engine = torimemo_enrich::AnthropicLabeller::from_env(Some(labeller))?;
                println!("labelling with {labeller}");
                run_labelling(&mut store, &engine, limit)
            }?;
            println!(
                "\rdone: {} labelled, {} skipped, {} failed ({} total)",
                summary.labelled,
                summary.skipped,
                summary.failed,
                summary.total()
            );
            Ok(())
        }
        Command::ExportTraining { labeller, out } => {
            let jsonl = torimemo_enrich::export_training_set(&store, labeller)?;
            match out {
                Some(path) => {
                    std::fs::write(path, format!("{jsonl}\n"))?;
                    println!("wrote {} examples to {}", jsonl.lines().count(), path.display());
                }
                None => println!("{jsonl}"),
            }
            Ok(())
        }
        Command::Tags => {
            for (tag, count) in store.tag_counts()? {
                println!("{count:>5}  {tag}");
            }
            Ok(())
        }
        Command::Train { teacher, out, min_support } => {
            let provider = embedder(&cli.model)?;
            let examples = training_examples(&store, teacher, provider.model())?;
            println!("{} labelled examples with embeddings", examples.len());

            let config = torimemo_classify::Config {
                min_support: *min_support,
                ..torimemo_classify::Config::default()
            };
            let (classifier, report) =
                torimemo_classify::train(&examples, provider.model(), teacher, &config)?;

            std::fs::write(out, classifier.to_json()?)?;
            print_report(&report, out);
            Ok(())
        }
        Command::Suggest { url, model_path, threshold } => {
            let bookmark = store
                .bookmark_by_url(url)?
                .ok_or_else(|| Error::msg("that URL is not in the store"))?;
            let classifier =
                torimemo_classify::Classifier::from_json(&std::fs::read_to_string(model_path)?)?;

            let provider = embedder(&cli.model)?;
            let text = torimemo_embed::embed_text(&bookmark);
            let embedding = torimemo_embed::Embedder::embed(&provider, &text)?;

            println!("{}", bookmark.title.as_deref().unwrap_or(&bookmark.canonical_url));
            for prediction in classifier.predict(&embedding.vector, *threshold)? {
                println!("  {:.3}  {}", prediction.probability, prediction.tag);
            }
            Ok(())
        }
        Command::Prune { reasons, apply, verbose } => prune(&mut store, reasons, *apply, *verbose),
        Command::Embed { batch } => {
            let provider = embedder(&cli.model)?;
            println!("embedding with {}", provider.model());
            let embedded = backfill(&store, &provider, *batch, |done| {
                progress_line(done, 256, &format!("{done} embedded"));
            })?;
            println!("\rembedded {embedded} bookmarks");
            Ok(())
        }
        Command::Recall { query, limit, floor } => {
            let provider = embedder(&cli.model)?;
            let matches = rank_by_similarity(&store, &provider, query, *limit, *floor)?;
            if matches.is_empty() {
                println!("no matches (is `torimemo embed` done?)");
            }
            for found in &matches {
                let title = found.bookmark.title.as_deref().unwrap_or("(no title)");
                println!("{:.3}  {title}\n       {}", found.score, found.bookmark.canonical_url);
            }
            Ok(())
        }
        Command::Search { query, limit } => {
            let results = store.search(query, *limit)?;
            if results.is_empty() {
                println!("no matches");
            }
            for bookmark in &results {
                let title = bookmark.title.as_deref().unwrap_or("(no title)");
                println!("{title}\n  {}\n", bookmark.canonical_url);
            }
            Ok(())
        }
        Command::Token { action } => match action {
            TokenAction::Issue { name, scope } => {
                let scope = match scope.as_str() {
                    "read" => torimemo_core::Scope::Read,
                    "read-write" | "read_write" => torimemo_core::Scope::ReadWrite,
                    other => {
                        return Err(Error::msg(format!(
                            "unknown scope {other}; use `read` or `read-write`"
                        )));
                    }
                };
                let issued = store.issue_token(name, scope)?;
                println!("id:    {}", issued.id);
                println!("name:  {}", issued.name);
                println!("scope: {}", issued.scope.as_str());
                println!("token: {}", issued.token);
                println!("\nThis is the only time the token is shown. Store it now.");
                Ok(())
            }
            TokenAction::List => {
                let tokens = store.list_tokens()?;
                if tokens.is_empty() {
                    println!("no tokens; the registry is open to any local caller");
                }
                for info in tokens {
                    let state = info.revoked_at.as_ref().map_or("live", |_| "revoked");
                    println!("{}  {:<16} {:<11} {state}", info.id, info.name, info.scope.as_str());
                }
                Ok(())
            }
            TokenAction::Revoke { id } => {
                if store.revoke_token(id)? {
                    println!("revoked {id}");
                } else {
                    println!("nothing to revoke: {id} is unknown or already revoked");
                }
                Ok(())
            }
        },
        Command::Serve { port } => {
            // Loading the ONNX model can take tens of seconds on first run,
            // because fastembed downloads ~127MB before it can answer
            // anything. Saying so is the difference between a container that
            // looks hung and one that is visibly working — and the reason the
            // deployment mounts the cache as a volume, so this happens once
            // rather than on every restart.
            println!("loading the {} embedding model...", cli.model);
            let provider = embedder(&cli.model)?;
            let state = torimemo_api::AppState::new(store, provider);
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(torimemo_api::serve(state, *port))
        }
        Command::Stats => {
            let stats = store.stats()?;
            println!("bookmarks   {}", stats.bookmarks);
            println!("captures    {}", stats.captures);
            println!("domains     {}", stats.domains);
            println!("with title  {}", stats.with_title);
            println!("embedded    {}", stats.embedded);
            println!("events      {}", stats.events);
            let fetch = store.fetch_summary()?;
            if !fetch.is_empty() {
                println!("\nenrichment");
                for (status, count) in fetch {
                    println!("  {count:>5}  {status}");
                }
            }
            println!("\ntop domains");
            for (domain, count) in store.top_domains(10)? {
                println!("  {count:>5}  {domain}");
            }
            Ok(())
        }
        Command::Repeats { limit } => {
            for bookmark in store.most_captured(*limit)? {
                println!("{:>3}x  {}", bookmark.capture_count, bookmark.canonical_url);
            }
            Ok(())
        }
    }
}

/// Reviews and optionally removes bookmarks that are no longer worth keeping.
///
/// Defaults to the objective categories only. The rest rest on an inference —
/// that a job posting is filled, that a listing is sold — which is usually but
/// not always right, so enabling them is the operator's decision.
fn prune(store: &mut Store, reasons: &[String], apply: bool, verbose: bool) -> Result<()> {
    let enabled = resolve_reasons(reasons)?;

    let mut candidates: Vec<torimemo_core::Candidate> = store
        .all_with_fetch_status()?
        .into_iter()
        .filter_map(|(bookmark, status)| {
            let reason = torimemo_core::prune::classify(&bookmark, status.as_deref())?;
            enabled.contains(&reason).then_some(torimemo_core::Candidate {
                id: bookmark.id,
                url: bookmark.canonical_url,
                title: bookmark.title,
                reason,
            })
        })
        .collect();

    // Duplicates are a property of the corpus rather than of one bookmark, so
    // they are found by query and merged in rather than classified per row.
    if enabled.contains(&torimemo_core::Reason::Duplicate) {
        let already: std::collections::HashSet<i64> =
            candidates.iter().map(|candidate| candidate.id).collect();
        for id in store.duplicate_title_ids()? {
            if already.contains(&id) {
                continue;
            }
            if let Some(bookmark) = store.bookmark(id)? {
                candidates.push(torimemo_core::Candidate {
                    id,
                    url: bookmark.canonical_url,
                    title: bookmark.title,
                    reason: torimemo_core::Reason::Duplicate,
                });
            }
        }
    }

    let total = store.stats()?.bookmarks;
    if candidates.is_empty() {
        println!("nothing to prune across {total} bookmarks");
        return Ok(());
    }

    report_candidates(&candidates, total, verbose);

    if !apply {
        println!("\nnothing was deleted. re-run with --apply to remove these.");
        return Ok(());
    }

    let ids: Vec<i64> = candidates.iter().map(|candidate| candidate.id).collect();
    let deleted = store.delete_bookmarks(&ids)?;
    println!("\ndeleted {deleted}; {} bookmarks remain", store.stats()?.bookmarks);
    Ok(())
}

/// Turns `--reason` flags into the set of enabled categories.
fn resolve_reasons(requested: &[String]) -> Result<Vec<torimemo_core::Reason>> {
    use torimemo_core::Reason;

    if requested.is_empty() {
        return Ok(Reason::all().iter().copied().filter(|reason| reason.is_objective()).collect());
    }
    if requested.iter().any(|value| value == "all") {
        return Ok(Reason::all().to_vec());
    }

    requested
        .iter()
        .map(|value| {
            Reason::parse(value).ok_or_else(|| {
                let names: Vec<&str> = Reason::all().iter().map(|r| r.as_str()).collect();
                Error::msg(format!("unknown category {value}; try one of: {}", names.join(", ")))
            })
        })
        .collect()
}

/// Prints the candidates grouped by reason.
fn report_candidates(candidates: &[torimemo_core::Candidate], total: i64, verbose: bool) {
    use torimemo_core::Reason;

    println!("{} of {total} bookmarks match:\n", candidates.len());

    for reason in Reason::all() {
        let matching: Vec<&torimemo_core::Candidate> =
            candidates.iter().filter(|candidate| candidate.reason == *reason).collect();
        if matching.is_empty() {
            continue;
        }

        println!("{:<16} {:>5}   {}", reason.as_str(), matching.len(), reason.explanation());

        // A few examples per category by default: enough to judge whether the
        // rule is behaving, without burying the summary.
        let shown = if verbose { matching.len() } else { 3.min(matching.len()) };
        for candidate in &matching[..shown] {
            let label = candidate.title.as_deref().unwrap_or(&candidate.url);
            println!("    {}", label.chars().take(72).collect::<String>());
        }
        if matching.len() > shown {
            println!("    ... and {} more", matching.len() - shown);
        }
        println!();
    }
}

/// Collects every labelled bookmark that also has a vector.
///
/// The join is the point: a bookmark labelled but not embedded has no
/// features, and one embedded but not labelled has no target. Only the
/// intersection can train.
fn training_examples(
    store: &Store,
    teacher: &str,
    embedding_model: &str,
) -> Result<Vec<torimemo_classify::Example>> {
    let vectors: std::collections::HashMap<i64, Vec<f32>> =
        store.embeddings(embedding_model)?.into_iter().collect();

    Ok(store
        .labelled(teacher)?
        .into_iter()
        .filter_map(|(bookmark, labels)| {
            vectors
                .get(&bookmark.id)
                .map(|vector| torimemo_classify::Example { vector: vector.clone(), labels })
        })
        .collect())
}

/// Prints a training report.
fn print_report(report: &torimemo_classify::Report, path: &Path) {
    println!("trained on {} examples, evaluated on {}", report.trained_on, report.evaluated_on);
    println!("{} labels fitted, macro F1 {:.3}\n", report.labels, report.macro_f1);

    println!("{:<20} {:>7} {:>6} {:>6} {:>6}", "label", "support", "prec", "rec", "F1");
    for entry in &report.per_label {
        println!(
            "{:<20} {:>7} {:>6.2} {:>6.2} {:>6.2}",
            entry.label, entry.support, entry.precision, entry.recall, entry.f1
        );
    }

    if !report.skipped.is_empty() {
        println!("\nnot fitted (too few examples):");
        for (label, support) in &report.skipped {
            println!("  {label} ({support})");
        }
    }
    println!("\nwrote {}", path.display());
}

/// Whether stdout is a terminal.
///
/// Carriage-return progress only overwrites on a terminal; piped to a file it
/// appends every update, turning a 2,000-row pass into a megabyte of log. When
/// stdout is redirected, progress is printed sparsely instead.
fn is_terminal() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

/// Prints a progress line, overwriting in place on a terminal and emitting
/// only every `every` updates otherwise.
fn progress_line(done: usize, every: usize, line: &str) {
    if is_terminal() {
        print!("\r  {line}          ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
    } else if done.is_multiple_of(every) {
        println!("  {line}");
    }
}

/// Runs a labelling pass, printing progress.
fn run_labelling(
    store: &mut Store,
    labeller: &impl torimemo_enrich::Labeller,
    limit: usize,
) -> Result<torimemo_enrich::labelrun::Summary> {
    torimemo_enrich::labelrun::run(store, labeller, limit, |summary| {
        progress_line(
            summary.total(),
            50,
            &format!(
                "{} labelled, {} skipped, {} failed",
                summary.labelled, summary.skipped, summary.failed
            ),
        );
    })
}

/// Builds the embedding provider named on the command line.
fn embedder(model: &str) -> Result<Provider> {
    if model == "deterministic" {
        return Ok(Provider::deterministic());
    }
    #[cfg(feature = "local-embeddings")]
    {
        Provider::local(model)
    }
    #[cfg(not(feature = "local-embeddings"))]
    {
        Err(Error::msg(format!(
            "this build has no local embedding support; pass --model deterministic (asked for {model})"
        )))
    }
}

/// Resolves the store path, creating the parent directory if needed.
fn database_path(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(path.to_path_buf());
    }
    let home = std::env::var_os("HOME").ok_or_else(|| Error::msg("$HOME is not set; pass --db"))?;
    let directory = PathBuf::from(home).join(".torimemo");
    std::fs::create_dir_all(&directory)?;
    Ok(directory.join("torimemo.db"))
}

fn import_vault(store: &mut Store, path: &Path) -> Result<()> {
    let entries = std::fs::read_dir(path)?;

    let mut total = torimemo_core::BatchOutcome::default();

    for entry in entries.flatten() {
        let file = entry.path();
        if file.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let name = file.file_name().and_then(|name| name.to_str()).unwrap_or("vault");

        // The file's modification time is the closest thing to a capture date
        // these dumps carry. It is wrong in detail but right in ordering, and
        // ordering is what recency features need.
        let captured_at = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map_or_else(|_| chrono::Utc::now(), chrono::DateTime::<chrono::Utc>::from);

        let captures = import::from_markdown(&text, name, captured_at);
        if captures.is_empty() {
            continue;
        }

        let outcome = store.ingest_batch(&captures)?;
        println!(
            "{name}: {} new, {} merged, {} skipped",
            outcome.created,
            outcome.merged,
            outcome.skipped.len()
        );
        total.created += outcome.created;
        total.merged += outcome.merged;
        total.skipped.extend(outcome.skipped);
    }

    report(&total);
    Ok(())
}

fn import_browser(store: &mut Store, path: &Path) -> Result<()> {
    let html = std::fs::read_to_string(path)?;
    let captures = import::from_netscape(&html, Source::BrowserImport);
    println!("found {} bookmarks", captures.len());
    let outcome = store.ingest_batch(&captures)?;
    report(&outcome);
    Ok(())
}

fn report(outcome: &torimemo_core::BatchOutcome) {
    println!(
        "\n{} new, {} merged, {} skipped",
        outcome.created,
        outcome.merged,
        outcome.skipped.len()
    );
    for (url, reason) in outcome.skipped.iter().take(10) {
        let truncated: String = url.chars().take(70).collect();
        println!("  skipped {truncated}: {reason}");
    }
    if outcome.skipped.len() > 10 {
        println!("  ... and {} more", outcome.skipped.len() - 10);
    }
}
