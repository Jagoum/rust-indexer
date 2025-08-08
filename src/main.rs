use std::{fs, path::PathBuf};
use anyhow::{Context, Result};
use clap::Parser;
use neo4rs::*;
use syn::{Expr, ExprCall, ExprPath, ExprStruct, Item, Stmt};
use walkdir::WalkDir;

/// A Rust codebase indexer for Neo4j.
/// Analyzes a Rust project and stores its structure and relationships in a graph database.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to the Rust project directory to index.
    #[arg(short, long)]
    path: PathBuf,

    /// URI for the Neo4j database.
    #[arg(long, env = "NEO4J_URI")]
    uri: String,

    /// Username for the Neo4j database.
    #[arg(short, long, env = "NEO4J_USER")]
    user: String,

    /// Password for the Neo4j database.
    #[arg(long, env = "NEO4J_PASS")]
    password: String,
}

enum Interaction {
    FunctionCall(String),
    StructInstantiation(String),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Load .env file for fallback configuration
    dotenv::dotenv().ok();
    let args = Cli::parse();

    let project_name = args
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .context("Project path must have a valid directory name")?
        .to_string();

    println!("Connecting to Neo4j at {}...", args.uri);
    let graph = Graph::new(&args.uri, &args.user, &args.password).await?;
    println!("✅ Connected to Neo4j.");

    println!("Indexing project: {}", project_name);
    graph
        .run(query("MERGE (p:Project {name: $name})").param("name", &*project_name))
        .await?;

    for entry in WalkDir::new(&args.path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let path = entry.path();
        let file_path = path.to_string_lossy().to_string();
        println!("Processing: {}", file_path);

        let code = fs::read_to_string(path)?;

        graph
            .run(
                query(
                    "
                    MATCH (p:Project {name: $project})
                    MERGE (f:File {path: $path})
                    MERGE (p)-[:CONTAINS_FILE]->(f)
                ",
                )
                .param("project", &*project_name)
                .param("path", &*file_path),
            )
            .await?;

        if let Ok(ast) = syn::parse_file(&code) {
            process_ast(&graph, &project_name, &file_path, ast).await?;
        }
    }

    println!("✅ Indexing complete for project: {}!", project_name);
    Ok(())
}

async fn process_ast(graph: &Graph, project: &str, file_path: &str, ast: syn::File) -> Result<()> {
    for item in ast.items {
        match item {
            Item::Fn(item_fn) => {
                let func_name = item_fn.sig.ident.to_string();
                graph
                    .run(
                        query(
                            "
                            MATCH (f:File {path: $path})
                            MERGE (fn:Function {name: $name, project: $project})
                            MERGE (f)-[:CONTAINS]->(fn)
                        ",
                        )
                        .param("path", file_path)
                        .param("name", &*func_name)
                        .param("project", project),
                    )
                    .await?;

                let mut interactions = Vec::new();
                for stmt in &item_fn.block.stmts {
                    find_interactions_in_stmt(stmt, &mut interactions);
                }

                for interaction in interactions {
                    match interaction {
                        Interaction::FunctionCall(callee_name) => {
                            graph
                                .run(
                                    query(
                                        "
                                        MATCH (caller:Function {name: $caller, project: $project})
                                        MERGE (callee:Function {name: $callee, project: $project})
                                        MERGE (caller)-[:CALLS]->(callee)
                                    ",
                                    )
                                    .param("caller", &*func_name)
                                    .param("callee", &*callee_name)
                                    .param("project", project),
                                )
                                .await?;
                        }
                        Interaction::StructInstantiation(struct_name) => {
                            graph
                                .run(
                                    query(
                                        "
                                        MATCH (caller:Function {name: $caller, project: $project})
                                        MERGE (s:Struct {name: $struct, project: $project})
                                        MERGE (caller)-[:INSTANTIATES]->(s)
                                    ",
                                    )
                                    .param("caller", &*func_name)
                                    .param("struct", &*struct_name)
                                    .param("project", project),
                                )
                                .await?;
                        }
                    }
                }
            }
            Item::Struct(item_struct) => {
                let struct_name = item_struct.ident.to_string();
                graph
                    .run(
                        query(
                            "
                            MATCH (f:File {path: $path})
                            MERGE (s:Struct {name: $name, project: $project})
                            MERGE (f)-[:CONTAINS]->(s)
                        ",
                        )
                        .param("path", file_path)
                        .param("name", &*struct_name)
                        .param("project", project),
                    )
                    .await?;
            }
            Item::Trait(item_trait) => {
                let trait_name = item_trait.ident.to_string();
                 graph
                    .run(
                        query(
                            "
                            MATCH (f:File {path: $path})
                            MERGE (t:Trait {name: $name, project: $project})
                            MERGE (f)-[:CONTAINS]->(t)
                        ",
                        )
                        .param("path", file_path)
                        .param("name", &*trait_name)
                        .param("project", project),
                    )
                    .await?;
            }
            Item::Impl(item_impl) => {
                if let Some(trait_path) = item_impl.trait_.as_ref().map(|t| &t.1) {
                    let struct_type = &*item_impl.self_ty;
                    if let (Some(trait_ident), Some(struct_ident)) = (trait_path.segments.last(), get_ident_from_type(struct_type)) {
                         graph
                            .run(
                                query(
                                    "
                                    MERGE (s:Struct {name: $struct, project: $project})
                                    MERGE (t:Trait {name: $trait, project: $project})
                                    MERGE (s)-[:IMPLEMENTS]->(t)
                                ",
                                )
                                .param("struct", &*struct_ident)
                                .param("trait", &*trait_ident.ident.to_string())
                                .param("project", project),
                            )
                            .await?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn get_ident_from_type(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return Some(segment.ident.to_string());
        }
    }
    None
}

fn find_interactions_in_stmt(stmt: &Stmt, interactions: &mut Vec<Interaction>) {
    match stmt {
        Stmt::Local(local) => {
            if let Some(init) = &local.init {
                find_interactions_in_expr(&init.expr, interactions);
            }
        }
        Stmt::Expr(expr, _) => {
            find_interactions_in_expr(expr, interactions);
        }
        _ => {}
    }
}

fn find_interactions_in_expr(expr: &Expr, interactions: &mut Vec<Interaction>) {
    match expr {
        Expr::Call(ExprCall { func, .. }) => {
            if let Expr::Path(ExprPath { path, .. }) = &**func {
                if let Some(ident) = path.get_ident() {
                    interactions.push(Interaction::FunctionCall(ident.to_string()));
                }
            }
        }
        Expr::Struct(ExprStruct { path, .. }) => {
            if let Some(ident) = path.get_ident() {
                interactions.push(Interaction::StructInstantiation(ident.to_string()));
            }
        }
        Expr::Block(block) => {
            for stmt in &block.block.stmts {
                find_interactions_in_stmt(stmt, interactions);
            }
        }
        Expr::If(expr_if) => {
            for stmt in &expr_if.then_branch.stmts {
                find_interactions_in_stmt(stmt, interactions);
            }
            if let Some((_, else_expr)) = &expr_if.else_branch {
                find_interactions_in_expr(else_expr, interactions);
            }
        }
        _ => {}
    }
}