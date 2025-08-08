# Rust Codebase Indexer for Neo4j

This is a command-line tool written in Rust to analyze a Rust project's source code and populate a [Neo4j](https://neo4j.com/) graph database with its structure and relationships. It allows you to query your codebase as a graph, enabling powerful analysis of dependencies, function calls, and type relationships.

This tool is designed to be multi-project aware. You can run it on several different codebases, and it will keep them separated within the same database, anchored by a top-level `:Project` node.

## Features

-   **Multi-Project Support:** Indexes multiple projects into the same database without conflicts.
-   **AST Parsing:** Uses the `syn` crate to parse Rust source files into an Abstract Syntax Tree for accurate analysis.
-   **Rich Graph Model:** Creates a detailed graph model of your codebase, including:
    -   `:Project` nodes to represent each codebase.
    -   `:File` nodes for every `.rs` source file.
    -   `:Function`, `:Struct`, and `:Trait` nodes.
    -   Relationships like `:CALLS`, `:INSTANTIATES`, and `:IMPLEMENTS`.

## The Graph Model

The indexer creates the following entities in your Neo4j database:

-   **Nodes:**
    -   `(:Project {name: String})`: A top-level node for each indexed project.
    -   `(:File {path: String})`: Represents a single `.rs` file.
    -   `(:Function {name: String, project: String})`: A function definition.
    -   `(:Struct {name: String, project: String})`: A struct definition.
    -   `(:Trait {name: String, project: String})`: A trait definition.
-   **Relationships:**
    -   `(:Project)-[:CONTAINS_FILE]->(:File)`
    -   `(:File)-[:CONTAINS]->(:Function | :Struct | :Trait)`
    -   `(:Function)-[:CALLS]->(:Function)`
    -   `(:Function)-[:INSTANTIATES]->(:Struct)`
    -   `(:Struct)-[:IMPLEMENTS]->(:Trait)`

## Prerequisites

1.  **Rust:** The tool is built with Rust. You'll need `rustc` and `cargo` installed.
2.  **Neo4j:** A running Neo4j database instance.

## Configuration

The tool is configured via command-line arguments. However, for convenience, you can place database credentials in a `.env` file in the project root, and the tool will use them as a fallback if the corresponding command-line arguments are not provided.

Create a `.env` file in the root of this project:

```
# .env

# Neo4j Database Configuration
NEO4J_URI="bolt://localhost:7687"
NEO4J_USER="neo4j"
NEO4J_PASS="your_secret_password"
```

## Usage

Run the indexer from the command line using `cargo run`. You must provide the path to the Rust project you wish to index.

### Basic Usage

The only required argument is `--path`. If your `.env` file is configured, this is all you need:

```bash
cargo run -- --path /path/to/your/rust/project
```

### Full Command-Line Options

You can override the `.env` configuration by providing the database details as arguments.

```bash
cargo run -- \
    --path /path/to/your/rust/project \
    --uri bolt://your-neo4j-host:7687 \
    --user my_user \
    --password my_secret_password
```

### Help

To see a full list of all command-line options, run:

```bash
cargo run -- --help
```
