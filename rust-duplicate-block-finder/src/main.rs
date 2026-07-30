use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::ToTokens;
use serde::Serialize;
use sha2::{Digest, Sha256};
use syn::{
    visit::{self, Visit},
    Block, FnArg, ImplItemFn, ItemFn, PatIdent, ReturnType, Signature, TraitItemFn,
};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Parser)]
#[command(about = "Find structurally identical Rust functions and nested blocks")]
struct Args {
    /// Root directory of the Rust project to scan.
    #[arg(default_value = ".")]
    root: PathBuf,

    /// Minimum inclusive source-line count for a recorded block.
    #[arg(long, default_value_t = 5)]
    min_lines: usize,

    /// Minimum normalized representation length for a recorded block.
    #[arg(long, default_value_t = 80)]
    min_chars: usize,

    /// Minimum inclusive source-line count for duplicate function candidates.
    #[arg(long, default_value_t = 5)]
    function_min_lines: usize,

    /// Minimum normalized body length for duplicate function candidates.
    #[arg(long, default_value_t = 80)]
    function_min_chars: usize,

    /// Omit functions below the function thresholds from functions.csv too.
    #[arg(long)]
    exclude_small_functions_from_inventory: bool,

    /// Directory in which CSV reports are written.
    #[arg(long, default_value = "duplicate-report")]
    output: PathBuf,

    /// Include tests, benches and examples directories.
    #[arg(long)]
    include_tests: bool,

    /// Minimum structured similarity score (0.0 through 1.0).
    #[arg(long, default_value_t = 0.80, value_parser = parse_similarity_threshold)]
    similarity_threshold: f64,

    /// Maximum number of rows written to similar-functions.csv.
    #[arg(long, default_value_t = 10_000)]
    max_similar_pairs: usize,

    /// Skip similarity feature extraction and pair comparison.
    #[arg(long)]
    no_similarity: bool,
}

#[derive(Debug, Clone)]
struct FunctionContext {
    id: usize,
    name: String,
    kind: &'static str,
    locals: LocalBindings,
}

#[derive(Debug, Serialize, Clone)]
struct FunctionRow {
    function_id: usize,
    file: String,
    kind: String,
    name: String,
    signature: String,
    return_type: String,
    start_line: usize,
    end_line: usize,
    line_count: usize,
    normalized_chars: usize,
    body_hash: String,
    signature_hash: String,
    hash: String,
    relevant_for_duplicates: bool,
    duplicate_count: usize,
    group_id: Option<String>,
    match_kind: Option<String>,
    comparison_basis: String,
    #[serde(skip)]
    similarity_features: Option<SimilarityFeatures>,
}

#[derive(Debug, Clone, Default)]
struct SimilarityFeatures {
    body_shingles: HashSet<String>,
    signature_shingles: HashSet<String>,
    identifiers: BTreeSet<String>,
    literals: BTreeSet<String>,
    control_flow: [usize; CONTROL_FLOW_TOKENS.len()],
}

#[derive(Debug, Serialize, Clone)]
struct SimilarFunctionRow {
    pair_id: String,
    left_function_id: usize,
    left_file: String,
    left_name: String,
    left_start_line: usize,
    right_function_id: usize,
    right_file: String,
    right_name: String,
    right_start_line: usize,
    similarity_percent: f64,
    body_similarity_percent: f64,
    signature_similarity_percent: f64,
    identifier_similarity_percent: f64,
    control_flow_similarity_percent: f64,
    literal_similarity_percent: f64,
    match_kind: String,
    comparison_basis: String,
    identifiers_only_left: String,
    identifiers_only_right: String,
    literals_only_left: String,
    literals_only_right: String,
    #[serde(skip)]
    raw_similarity: f64,
}

#[derive(Debug, Serialize, Clone)]
struct BlockRow {
    block_id: usize,
    parent_block_id: Option<usize>,
    function_id: usize,
    file: String,
    function_kind: String,
    function_name: String,
    depth: usize,
    start_line: usize,
    end_line: usize,
    line_count: usize,
    normalized_chars: usize,
    hash: String,
    duplicate_count: usize,
    group_id: Option<String>,
    match_kind: Option<String>,
    comparison_basis: String,
}

#[derive(Debug, Clone, Default)]
struct LocalBindings {
    indexes: HashMap<String, usize>,
}

impl LocalBindings {
    fn insert(&mut self, name: String) {
        let next = self.indexes.len();
        self.indexes.entry(name).or_insert(next);
    }

    fn replacement(&self, name: &str) -> Option<String> {
        self.indexes.get(name).map(|index| format!("LOCAL_{index}"))
    }
}

#[derive(Default)]
struct BindingCollector {
    bindings: LocalBindings,
}

impl<'ast> Visit<'ast> for BindingCollector {
    fn visit_pat_ident(&mut self, node: &'ast PatIdent) {
        self.bindings.insert(node.ident.to_string());
        visit::visit_pat_ident(self, node);
    }
}

struct Collector<'a> {
    file: &'a Path,
    min_lines: usize,
    min_chars: usize,
    function_min_lines: usize,
    function_min_chars: usize,
    collect_similarity_features: bool,
    next_function_id: &'a mut usize,
    next_block_id: &'a mut usize,
    functions: &'a mut Vec<FunctionRow>,
    blocks: &'a mut Vec<BlockRow>,
    function_stack: Vec<FunctionContext>,
    block_stack: Vec<usize>,
}

impl<'a> Collector<'a> {
    fn enter_function<F>(
        &mut self,
        name: String,
        kind: &'static str,
        signature: &Signature,
        block: &Block,
        visit_body: F,
    ) where
        F: FnOnce(&mut Self),
    {
        let function_id = *self.next_function_id;
        *self.next_function_id += 1;

        let locals = collect_local_bindings(signature, block);
        let (start_line, end_line) = block_lines(block);
        let line_count = inclusive_line_count(start_line, end_line);
        let normalized_body = normalize_tokens(block.to_token_stream(), &locals);
        let normalized_signature = normalize_signature(signature, &locals);
        let body_hash = sha256(&normalized_body);
        let signature_hash = sha256(&normalized_signature);
        let comparison_hash = sha256(&format!("{normalized_signature}\n{normalized_body}"));
        let relevant = line_count >= self.function_min_lines
            && normalized_body.len() >= self.function_min_chars;
        let similarity_features = (self.collect_similarity_features && relevant).then(|| {
            build_similarity_features(
                signature,
                block,
                &locals,
                &normalized_signature,
                &normalized_body,
            )
        });

        self.functions.push(FunctionRow {
            function_id,
            file: self.file.display().to_string(),
            kind: kind.to_string(),
            name: name.clone(),
            signature: signature.to_token_stream().to_string(),
            return_type: return_type_text(&signature.output),
            start_line,
            end_line,
            line_count,
            normalized_chars: normalized_body.len(),
            body_hash,
            signature_hash,
            hash: comparison_hash,
            relevant_for_duplicates: relevant,
            duplicate_count: 0,
            group_id: None,
            match_kind: None,
            comparison_basis: "normalized_signature+normalized_body".to_string(),
            similarity_features,
        });

        let outer_blocks = std::mem::take(&mut self.block_stack);
        self.function_stack.push(FunctionContext {
            id: function_id,
            name,
            kind,
            locals,
        });
        visit_body(self);
        self.function_stack.pop();
        self.block_stack = outer_blocks;
    }
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.enter_function(
            node.sig.ident.to_string(),
            "free_fn",
            &node.sig,
            &node.block,
            |this| this.visit_block(&node.block),
        );
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.enter_function(
            node.sig.ident.to_string(),
            "method",
            &node.sig,
            &node.block,
            |this| this.visit_block(&node.block),
        );
    }

    fn visit_trait_item_fn(&mut self, node: &'ast TraitItemFn) {
        if let Some(default) = &node.default {
            self.enter_function(
                node.sig.ident.to_string(),
                "trait_default_method",
                &node.sig,
                default,
                |this| this.visit_block(default),
            );
        }
    }

    fn visit_block(&mut self, node: &'ast Block) {
        let Some(function) = self.function_stack.last().cloned() else {
            visit::visit_block(self, node);
            return;
        };

        let (start_line, end_line) = block_lines(node);
        let line_count = inclusive_line_count(start_line, end_line);
        let normalized = normalize_tokens(node.to_token_stream(), &function.locals);
        let should_record = line_count >= self.min_lines && normalized.len() >= self.min_chars;
        let parent_block_id = self.block_stack.last().copied();
        let block_id = if should_record {
            let id = *self.next_block_id;
            *self.next_block_id += 1;
            self.blocks.push(BlockRow {
                block_id: id,
                parent_block_id,
                function_id: function.id,
                file: self.file.display().to_string(),
                function_kind: function.kind.to_string(),
                function_name: function.name.clone(),
                depth: self.block_stack.len(),
                start_line,
                end_line,
                line_count,
                normalized_chars: normalized.len(),
                hash: sha256(&normalized),
                duplicate_count: 0,
                group_id: None,
                match_kind: None,
                comparison_basis: "normalized_block_body".to_string(),
            });
            Some(id)
        } else {
            None
        };

        if let Some(id) = block_id {
            self.block_stack.push(id);
        }
        visit::visit_block(self, node);
        if block_id.is_some() {
            self.block_stack.pop();
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.output)
        .with_context(|| format!("cannot create {}", args.output.display()))?;

    let mut functions = Vec::new();
    let mut blocks = Vec::new();
    let mut next_function_id = 1usize;
    let mut next_block_id = 1usize;
    let mut parsed_files = 0usize;
    let mut parse_errors = Vec::new();

    for entry in WalkDir::new(&args.root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_descend(entry, args.include_tests))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.path();
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                parse_errors.push(format!("{}: read error: {error}", path.display()));
                continue;
            }
        };
        let syntax = match syn::parse_file(&source) {
            Ok(syntax) => syntax,
            Err(error) => {
                parse_errors.push(format!("{}: parse error: {error}", path.display()));
                continue;
            }
        };

        parsed_files += 1;
        let mut collector = Collector {
            file: path,
            min_lines: args.min_lines,
            min_chars: args.min_chars,
            function_min_lines: args.function_min_lines,
            function_min_chars: args.function_min_chars,
            collect_similarity_features: !args.no_similarity,
            next_function_id: &mut next_function_id,
            next_block_id: &mut next_block_id,
            functions: &mut functions,
            blocks: &mut blocks,
            function_stack: Vec::new(),
            block_stack: Vec::new(),
        };
        collector.visit_file(&syntax);
    }

    assign_function_groups(&mut functions);
    assign_block_groups(&mut blocks);
    let similar_functions = if args.no_similarity {
        None
    } else {
        Some(find_similar_pairs(
            &functions,
            args.similarity_threshold,
            args.max_similar_pairs,
        ))
    };

    functions.sort_by_key(|row| (row.file.clone(), row.start_line));
    blocks.sort_by_key(|row| (row.file.clone(), row.start_line, row.depth));

    let function_inventory: Vec<_> = functions
        .iter()
        .filter(|row| !args.exclude_small_functions_from_inventory || row.relevant_for_duplicates)
        .cloned()
        .collect();
    let mut duplicate_functions: Vec<_> = functions
        .iter()
        .filter(|row| row.relevant_for_duplicates && row.duplicate_count > 1)
        .cloned()
        .collect();
    let mut duplicate_blocks: Vec<_> = blocks
        .iter()
        .filter(|row| row.duplicate_count > 1)
        .cloned()
        .collect();
    duplicate_functions.sort_by_key(|row| (row.group_id.clone(), row.file.clone(), row.start_line));
    duplicate_blocks.sort_by_key(|row| (row.group_id.clone(), row.file.clone(), row.start_line));

    write_csv(args.output.join("functions.csv"), &function_inventory)?;
    write_csv(args.output.join("blocks.csv"), &blocks)?;
    write_csv(
        args.output.join("duplicate-functions.csv"),
        &duplicate_functions,
    )?;
    write_csv(args.output.join("duplicate-blocks.csv"), &duplicate_blocks)?;
    let similar_path = args.output.join("similar-functions.csv");
    if let Some(rows) = &similar_functions {
        write_similar_csv(similar_path, rows)?;
    } else if similar_path.exists() {
        fs::remove_file(similar_path)?;
    }

    let errors_path = args.output.join("errors.txt");
    if parse_errors.is_empty() {
        if errors_path.exists() {
            fs::remove_file(errors_path)?;
        }
    } else {
        fs::write(errors_path, parse_errors.join("\n"))?;
    }

    println!(
        "Scanned {parsed_files} Rust files; inventoried {} functions and recorded {} blocks.",
        function_inventory.len(),
        blocks.len()
    );
    println!(
        "Relevant duplicate rows: {} functions, {} blocks. Reports: {}",
        duplicate_functions.len(),
        duplicate_blocks.len(),
        args.output.display()
    );
    if let Some(rows) = &similar_functions {
        println!(
            "Similar function pairs: {} (threshold {:.0}%, cap {}).",
            rows.len(),
            args.similarity_threshold * 100.0,
            args.max_similar_pairs
        );
    } else {
        println!("Similarity analysis skipped (--no-similarity).");
    }

    Ok(())
}

fn collect_local_bindings(signature: &Signature, block: &Block) -> LocalBindings {
    let mut collector = BindingCollector::default();
    for input in &signature.inputs {
        if let FnArg::Typed(argument) = input {
            collector.visit_pat(&argument.pat);
        }
    }
    collector.visit_block(block);
    collector.bindings
}

fn normalize_signature(signature: &Signature, locals: &LocalBindings) -> String {
    let mut signature = signature.clone();
    signature.ident = syn::Ident::new("FUNCTION", signature.ident.span());
    normalize_tokens(signature.to_token_stream(), locals)
}

fn return_type_text(output: &ReturnType) -> String {
    match output {
        ReturnType::Default => "()".to_string(),
        ReturnType::Type(_, ty) => ty.to_token_stream().to_string(),
    }
}

fn should_descend(entry: &DirEntry, include_tests: bool) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    if matches!(
        name.as_ref(),
        ".git" | "target" | "node_modules" | ".idea" | ".vscode"
    ) {
        return false;
    }
    include_tests || !matches!(name.as_ref(), "tests" | "benches" | "examples")
}

fn block_lines(block: &Block) -> (usize, usize) {
    let start = block.brace_token.span.open().start().line;
    let end = block.brace_token.span.close().end().line;
    (start, end)
}

fn inclusive_line_count(start: usize, end: usize) -> usize {
    end.saturating_sub(start).saturating_add(1)
}

fn sha256(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

fn normalize_tokens(tokens: TokenStream, locals: &LocalBindings) -> String {
    fn push_stream(out: &mut String, stream: TokenStream, locals: &LocalBindings) {
        let tokens: Vec<_> = stream.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            match token {
                TokenTree::Ident(ident) => {
                    let text = ident.to_string();
                    let previous_is_member_access = index > 0
                        && matches!(&tokens[index - 1], TokenTree::Punct(p) if matches!(p.as_char(), '.' | ':'));
                    let next_is_macro_marker = matches!(
                        tokens.get(index + 1),
                        Some(TokenTree::Punct(p)) if p.as_char() == '!'
                    );
                    if !previous_is_member_access && !next_is_macro_marker {
                        if let Some(replacement) = locals.replacement(&text) {
                            out.push_str(&replacement);
                            out.push(' ');
                            continue;
                        }
                    }
                    out.push_str(&text);
                    out.push(' ');
                }
                TokenTree::Literal(_) => out.push_str("LIT "),
                TokenTree::Punct(punct) => {
                    out.push(punct.as_char());
                    out.push(' ');
                }
                TokenTree::Group(group) => push_group(out, group.clone(), locals),
            }
        }
    }

    fn push_group(out: &mut String, group: Group, locals: &LocalBindings) {
        let (open, close) = match group.delimiter() {
            Delimiter::Parenthesis => ('(', ')'),
            Delimiter::Brace => ('{', '}'),
            Delimiter::Bracket => ('[', ']'),
            Delimiter::None => ('<', '>'),
        };
        out.push(open);
        out.push(' ');
        push_stream(out, group.stream(), locals);
        out.push(close);
        out.push(' ');
    }

    let mut normalized = String::new();
    push_stream(&mut normalized, tokens, locals);
    normalized
}

const BODY_WEIGHT: f64 = 0.55;
const IDENTIFIER_WEIGHT: f64 = 0.20;
const SIGNATURE_WEIGHT: f64 = 0.15;
const CONTROL_FLOW_WEIGHT: f64 = 0.10;
const SHINGLE_SIZE: usize = 3;
const CONTROL_FLOW_TOKENS: [&str; 11] = [
    "if", "else", "match", "for", "while", "loop", "return", "break", "continue", "await", "?",
];
const SIMILARITY_BASIS: &str = "55% body token shingles + 20% relevant identifiers + 15% normalized signature shingles + 10% control flow; literals diagnostic only";

fn parse_similarity_threshold(value: &str) -> std::result::Result<f64, String> {
    let parsed = value.parse::<f64>().map_err(|_| {
        format!("similarity threshold must be a number from 0.0 through 1.0, got '{value}'")
    })?;
    if parsed.is_finite() && (0.0..=1.0).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(format!(
            "similarity threshold must be from 0.0 through 1.0, got '{value}'"
        ))
    }
}

fn build_similarity_features(
    signature: &Signature,
    block: &Block,
    locals: &LocalBindings,
    normalized_signature: &str,
    normalized_body: &str,
) -> SimilarityFeatures {
    let body_tokens: Vec<_> = normalized_body
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let signature_tokens: Vec<_> = normalized_signature
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let mut identifiers = BTreeSet::new();
    let mut literals = BTreeSet::new();
    collect_diagnostic_tokens(
        block.to_token_stream(),
        locals,
        &mut identifiers,
        &mut literals,
    );
    let mut control_flow = [0; CONTROL_FLOW_TOKENS.len()];
    for token in &body_tokens {
        if let Some(index) = CONTROL_FLOW_TOKENS
            .iter()
            .position(|candidate| *candidate == token)
        {
            control_flow[index] += 1;
        }
    }
    // Keep the signature parameter to make it explicit that all signature modifiers,
    // generics and where clauses originate from syn's complete Signature representation.
    let _ = signature;
    SimilarityFeatures {
        body_shingles: shingles(&body_tokens, SHINGLE_SIZE),
        signature_shingles: shingles(&signature_tokens, SHINGLE_SIZE),
        identifiers,
        literals,
        control_flow,
    }
}

fn collect_diagnostic_tokens(
    stream: TokenStream,
    locals: &LocalBindings,
    identifiers: &mut BTreeSet<String>,
    literals: &mut BTreeSet<String>,
) {
    let tokens: Vec<_> = stream.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(ident) => {
                let text = ident.to_string();
                let member_access = index > 0
                    && matches!(&tokens[index - 1], TokenTree::Punct(p) if matches!(p.as_char(), '.' | ':'));
                let macro_name = matches!(tokens.get(index + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!');
                if (member_access || macro_name || locals.replacement(&text).is_none())
                    && !is_ignored_identifier(&text)
                {
                    identifiers.insert(text);
                }
            }
            TokenTree::Literal(literal) => {
                literals.insert(literal.to_string());
            }
            TokenTree::Group(group) => {
                collect_diagnostic_tokens(group.stream(), locals, identifiers, literals);
            }
            TokenTree::Punct(_) => {}
        }
    }
}

fn is_ignored_identifier(value: &str) -> bool {
    matches!(
        value,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

fn shingles(tokens: &[String], size: usize) -> HashSet<String> {
    if tokens.is_empty() {
        return HashSet::new();
    }
    if tokens.len() < size {
        return HashSet::from([tokens.join(" ")]);
    }
    tokens
        .windows(size)
        .map(|window| window.join(" "))
        .collect()
}

fn jaccard<T: Eq + std::hash::Hash>(left: &HashSet<T>, right: &HashSet<T>) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let intersection = left.intersection(right).count();
    let union = left.len() + right.len() - intersection;
    intersection as f64 / union as f64
}

fn btree_jaccard<T: Ord>(left: &BTreeSet<T>, right: &BTreeSet<T>) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let intersection = left.intersection(right).count();
    let union = left.len() + right.len() - intersection;
    intersection as f64 / union as f64
}

fn control_similarity(
    left: &[usize; CONTROL_FLOW_TOKENS.len()],
    right: &[usize; CONTROL_FLOW_TOKENS.len()],
) -> f64 {
    let intersection: usize = left
        .iter()
        .zip(right)
        .map(|(left, right)| (*left).min(*right))
        .sum();
    let union: usize = left
        .iter()
        .zip(right)
        .map(|(left, right)| (*left).max(*right))
        .sum();
    if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    }
}

fn jaccard_size_upper_bound(left_len: usize, right_len: usize) -> f64 {
    let longer = left_len.max(right_len);
    if longer == 0 {
        1.0
    } else {
        left_len.min(right_len) as f64 / longer as f64
    }
}

fn similarity_upper_bound(left: &SimilarityFeatures, right: &SimilarityFeatures) -> f64 {
    BODY_WEIGHT * jaccard_size_upper_bound(left.body_shingles.len(), right.body_shingles.len())
        + IDENTIFIER_WEIGHT
            * jaccard_size_upper_bound(left.identifiers.len(), right.identifiers.len())
        + SIGNATURE_WEIGHT
            * jaccard_size_upper_bound(
                left.signature_shingles.len(),
                right.signature_shingles.len(),
            )
        + CONTROL_FLOW_WEIGHT
            * jaccard_size_upper_bound(
                left.control_flow.iter().sum(),
                right.control_flow.iter().sum(),
            )
}

fn percent(value: f64) -> f64 {
    (value.clamp(0.0, 1.0) * 10_000.0).round() / 100.0
}

fn set_difference(left: &BTreeSet<String>, right: &BTreeSet<String>) -> String {
    left.difference(right)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ")
}

fn function_order_key(row: &FunctionRow) -> (&str, usize, usize) {
    (&row.file, row.start_line, row.function_id)
}

fn find_similar_pairs(
    functions: &[FunctionRow],
    threshold: f64,
    max_pairs: usize,
) -> Vec<SimilarFunctionRow> {
    if max_pairs == 0 {
        return Vec::new();
    }
    let candidates: Vec<_> = functions
        .iter()
        .filter(|row| row.relevant_for_duplicates && row.similarity_features.is_some())
        .collect();
    let mut rows = Vec::new();
    for left_index in 0..candidates.len() {
        for right_index in (left_index + 1)..candidates.len() {
            let first = candidates[left_index];
            let second = candidates[right_index];
            let left_features = first.similarity_features.as_ref().unwrap();
            let right_features = second.similarity_features.as_ref().unwrap();
            let exact = first.hash == second.hash;
            if !exact
                && similarity_upper_bound(left_features, right_features) + f64::EPSILON < threshold
            {
                continue;
            }

            let body = jaccard(&left_features.body_shingles, &right_features.body_shingles);
            let identifiers =
                btree_jaccard(&left_features.identifiers, &right_features.identifiers);
            let signature = jaccard(
                &left_features.signature_shingles,
                &right_features.signature_shingles,
            );
            let control =
                control_similarity(&left_features.control_flow, &right_features.control_flow);
            let score = if exact {
                1.0
            } else {
                BODY_WEIGHT * body
                    + IDENTIFIER_WEIGHT * identifiers
                    + SIGNATURE_WEIGHT * signature
                    + CONTROL_FLOW_WEIGHT * control
            };
            if score + f64::EPSILON < threshold {
                continue;
            }

            let (left, right, lf, rf) = if function_order_key(first) <= function_order_key(second) {
                (first, second, left_features, right_features)
            } else {
                (second, first, right_features, left_features)
            };
            rows.push(SimilarFunctionRow {
                pair_id: String::new(),
                left_function_id: left.function_id,
                left_file: left.file.clone(),
                left_name: left.name.clone(),
                left_start_line: left.start_line,
                right_function_id: right.function_id,
                right_file: right.file.clone(),
                right_name: right.name.clone(),
                right_start_line: right.start_line,
                similarity_percent: percent(score),
                body_similarity_percent: percent(body),
                signature_similarity_percent: percent(signature),
                identifier_similarity_percent: percent(identifiers),
                control_flow_similarity_percent: percent(control),
                literal_similarity_percent: percent(btree_jaccard(&lf.literals, &rf.literals)),
                match_kind: if exact {
                    "exact_normalized"
                } else {
                    "similar_normalized"
                }
                .to_string(),
                comparison_basis: SIMILARITY_BASIS.to_string(),
                identifiers_only_left: set_difference(&lf.identifiers, &rf.identifiers),
                identifiers_only_right: set_difference(&rf.identifiers, &lf.identifiers),
                literals_only_left: set_difference(&lf.literals, &rf.literals),
                literals_only_right: set_difference(&rf.literals, &lf.literals),
                raw_similarity: score,
            });
            if rows.len() > max_pairs.saturating_mul(2) {
                sort_similar_pairs(&mut rows);
                rows.truncate(max_pairs);
            }
        }
    }
    sort_similar_pairs(&mut rows);
    rows.truncate(max_pairs);
    for (index, row) in rows.iter_mut().enumerate() {
        row.pair_id = format!("SIM-{:04}", index + 1);
    }
    rows
}

fn sort_similar_pairs(rows: &mut [SimilarFunctionRow]) {
    rows.sort_by(|left, right| {
        right
            .raw_similarity
            .total_cmp(&left.raw_similarity)
            .then_with(|| left.left_file.cmp(&right.left_file))
            .then_with(|| left.left_start_line.cmp(&right.left_start_line))
            .then_with(|| left.left_function_id.cmp(&right.left_function_id))
            .then_with(|| left.right_file.cmp(&right.right_file))
            .then_with(|| left.right_start_line.cmp(&right.right_start_line))
            .then_with(|| left.right_function_id.cmp(&right.right_function_id))
    });
}

fn assign_function_groups(rows: &mut [FunctionRow]) {
    assign_groups(
        rows,
        "FN",
        |row| row.hash.clone(),
        |row| row.relevant_for_duplicates,
        |row, count, group_id| {
            row.duplicate_count = count;
            row.group_id = group_id;
            row.match_kind = (count > 1).then(|| "exact_normalized".to_string());
        },
    );
}

fn assign_block_groups(rows: &mut [BlockRow]) {
    assign_groups(
        rows,
        "BLK",
        |row| row.hash.clone(),
        |_| true,
        |row, count, group_id| {
            row.duplicate_count = count;
            row.group_id = group_id;
            row.match_kind = (count > 1).then(|| "exact_normalized".to_string());
        },
    );
}

fn assign_groups<T, KF, IF, SF>(rows: &mut [T], prefix: &str, key: KF, include: IF, set: SF)
where
    KF: Fn(&T) -> String,
    IF: Fn(&T) -> bool,
    SF: Fn(&mut T, usize, Option<String>),
{
    let mut counts = HashMap::new();
    for row in rows.iter().filter(|row| include(row)) {
        *counts.entry(key(row)).or_insert(0usize) += 1;
    }

    let mut duplicate_keys: Vec<_> = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(key, count)| (key.clone(), *count))
        .collect();
    duplicate_keys.sort_by(|(left_key, left_count), (right_key, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_key.cmp(right_key))
    });
    let group_ids: HashMap<_, _> = duplicate_keys
        .into_iter()
        .enumerate()
        .map(|(index, (key, _))| (key, format!("{prefix}-{:04}", index + 1)))
        .collect();

    for row in rows {
        let row_key = key(row);
        let count = if include(row) {
            counts.get(&row_key).copied().unwrap_or(0)
        } else {
            0
        };
        set(row, count, group_ids.get(&row_key).cloned());
    }
}

fn write_similar_csv(path: PathBuf, rows: &[SimilarFunctionRow]) -> Result<()> {
    if !rows.is_empty() {
        return write_csv(path, rows);
    }
    let mut writer = csv::Writer::from_path(&path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    writer.write_record([
        "pair_id",
        "left_function_id",
        "left_file",
        "left_name",
        "left_start_line",
        "right_function_id",
        "right_file",
        "right_name",
        "right_start_line",
        "similarity_percent",
        "body_similarity_percent",
        "signature_similarity_percent",
        "identifier_similarity_percent",
        "control_flow_similarity_percent",
        "literal_similarity_percent",
        "match_kind",
        "comparison_basis",
        "identifiers_only_left",
        "identifiers_only_right",
        "literals_only_left",
        "literals_only_right",
    ])?;
    writer.flush()?;
    Ok(())
}

fn write_csv<T: Serialize>(path: PathBuf, rows: &[T]) -> Result<()> {
    let mut writer = csv::Writer::from_path(&path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_rows(source: &str) -> Vec<FunctionRow> {
        let file = syn::parse_file(source).unwrap();
        let mut functions = Vec::new();
        let mut blocks = Vec::new();
        let mut next_function_id = 1;
        let mut next_block_id = 1;
        let mut collector = Collector {
            file: Path::new("test.rs"),
            min_lines: 0,
            min_chars: 0,
            function_min_lines: 0,
            function_min_chars: 0,
            collect_similarity_features: true,
            next_function_id: &mut next_function_id,
            next_block_id: &mut next_block_id,
            functions: &mut functions,
            blocks: &mut blocks,
            function_stack: Vec::new(),
            block_stack: Vec::new(),
        };
        collector.visit_file(&file);
        functions
    }

    fn fingerprints(source: &str) -> Vec<(String, String, String)> {
        function_rows(source)
            .into_iter()
            .map(|row| (row.body_hash, row.signature_hash, row.hash))
            .collect()
    }

    fn pairs(source: &str, threshold: f64, cap: usize) -> Vec<SimilarFunctionRow> {
        let mut rows = function_rows(source);
        assign_function_groups(&mut rows);
        find_similar_pairs(&rows, threshold, cap)
    }

    #[test]
    fn renamed_local_variables_still_match() {
        let rows = fingerprints(
            "fn a(x: i32) -> i32 { let y = x + 1; y } fn b(q: i32) -> i32 { let z = q + 2; z }",
        );
        assert_eq!(rows[0].0, rows[1].0);
        assert_eq!(rows[0].2, rows[1].2);
    }

    #[test]
    fn different_fields_do_not_match() {
        let rows = fingerprints(
            "impl A { fn a(&self) -> i32 { self.left } fn b(&self) -> i32 { self.right } }",
        );
        assert_ne!(rows[0].0, rows[1].0);
        assert_ne!(rows[0].2, rows[1].2);
    }

    #[test]
    fn different_return_types_do_not_match() {
        let rows = fingerprints("fn a() -> i32 { value() } fn b() -> String { value() }");
        assert_eq!(rows[0].0, rows[1].0);
        assert_ne!(rows[0].1, rows[1].1);
        assert_ne!(rows[0].2, rows[1].2);
    }

    #[test]
    fn different_called_functions_do_not_match() {
        let rows = fingerprints("fn a() -> i32 { left() } fn b() -> i32 { right() }");
        assert_ne!(rows[0].0, rows[1].0);
    }

    #[test]
    fn renamed_parameters_and_locals_are_exact_similarity_matches() {
        let rows = pairs(
            "fn a(x: i32) -> i32 { let y = x + 1; y * 2 } fn b(q: i32) -> i32 { let z = q + 9; z * 7 }",
            0.8,
            10,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].similarity_percent, 100.0);
        assert_eq!(rows[0].match_kind, "exact_normalized");
    }

    #[test]
    fn changed_call_remains_similar_but_reduces_score() {
        let rows = pairs(
            "fn a(x: i32) -> i32 { let y = prepare(x); validate(y); audit(y); trace(y); let y = y + 1; let y = y * 2; let y = y - 3; normalize(y); finish(y) } fn b(q: i32) -> i32 { let z = prepare(q); validate(z); audit(z); trace(z); let z = z + 1; let z = z * 2; let z = z - 3; normalize(z); publish(z) }",
            0.8,
            10,
        );
        assert_eq!(rows.len(), 1);
        assert!(rows[0].similarity_percent >= 80.0);
        assert!(rows[0].similarity_percent < 100.0);
        assert_eq!(rows[0].match_kind, "similar_normalized");
    }

    #[test]
    fn member_name_matching_a_local_binding_is_preserved() {
        let rows = fingerprints(
            "fn a(obj: &A) -> i32 { let value = 1; obj.value + value } fn b(other: &A) -> i32 { let renamed = 2; other.value + renamed }",
        );
        assert_eq!(rows[0].0, rows[1].0);
        assert_eq!(rows[0].2, rows[1].2);
    }

    #[test]
    fn field_differences_are_reported_as_identifier_diagnostics() {
        let rows = pairs(
            "impl A { fn a(&self) -> i32 { let x = self.left; x + self.shared } fn b(&self) -> i32 { let x = self.right; x + self.shared } }",
            0.0,
            10,
        );
        assert_eq!(rows.len(), 1);
        assert!(rows[0].identifiers_only_left.contains("left"));
        assert!(rows[0].identifiers_only_right.contains("right"));
    }

    #[test]
    fn literal_differences_are_reported_even_for_exact_structure() {
        let rows = pairs(
            "fn a(x: i32) -> i32 { x + 10 } fn b(y: i32) -> i32 { y + 20 }",
            0.8,
            10,
        );
        assert_eq!(rows[0].similarity_percent, 100.0);
        assert!(rows[0].literal_similarity_percent < 100.0);
        assert_eq!(rows[0].literals_only_left, "10");
        assert_eq!(rows[0].literals_only_right, "20");
    }

    #[test]
    fn different_return_types_reduce_signature_similarity() {
        let rows = pairs(
            "fn a() -> i32 { value() } fn b() -> String { value() }",
            0.0,
            10,
        );
        assert!(rows[0].signature_similarity_percent < 100.0);
    }

    #[test]
    fn unrelated_functions_stay_below_default_threshold() {
        let rows = pairs(
            "fn a(x: i32) -> i32 { if x > 0 { calculate(x) } else { fallback() } } fn b() -> String { for item in entries() { println!(\"{}\", item); } String::new() }",
            0.8,
            10,
        );
        assert!(rows.is_empty());
    }

    #[test]
    fn cli_accepts_threshold_boundaries_and_rejects_invalid_values() {
        assert!(Args::try_parse_from(["tool", "--similarity-threshold", "0.0"]).is_ok());
        assert!(Args::try_parse_from(["tool", "--similarity-threshold", "1.0"]).is_ok());
        let error = Args::try_parse_from(["tool", "--similarity-threshold", "1.01"])
            .unwrap_err()
            .to_string();
        assert!(error.contains("0.0 through 1.0"));
        assert!(Args::try_parse_from(["tool", "--similarity-threshold", "NaN"]).is_err());
        assert!(Args::try_parse_from(["tool", "--similarity-threshold", "-0.01"]).is_err());
        assert!(Args::try_parse_from(["tool", "--similarity-threshold", "inf"]).is_err());
        assert!(
            Args::try_parse_from(["tool", "--no-similarity"])
                .unwrap()
                .no_similarity
        );
    }

    #[test]
    fn pair_output_has_no_self_comparisons_or_reversed_duplicates() {
        let rows = pairs(
            "fn a(x: i32) -> i32 { x + 1 } fn b(x: i32) -> i32 { x + 2 } fn c(x: i32) -> i32 { x + 3 }",
            0.0,
            10,
        );
        let unordered: HashSet<_> = rows
            .iter()
            .map(|row| {
                assert_ne!(row.left_function_id, row.right_function_id);
                let mut ids = [row.left_function_id, row.right_function_id];
                ids.sort();
                ids
            })
            .collect();
        assert_eq!(unordered.len(), rows.len());
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn maximum_similar_pair_count_is_enforced() {
        let rows = pairs(
            "fn a(x: i32) -> i32 { x + 1 } fn b(x: i32) -> i32 { x + 2 } fn c(x: i32) -> i32 { x + 3 }",
            0.0,
            2,
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pair_id, "SIM-0001");
        assert_eq!(rows[1].pair_id, "SIM-0002");
        assert!(rows[0].raw_similarity >= rows[1].raw_similarity);
    }

    #[test]
    fn similarity_prefilter_upper_bound_never_falls_below_actual_score() {
        let rows = function_rows(
            "fn a(x: i32) -> i32 { if x > 0 { left(x) } else { shared(x) } } fn b(y: i32) -> i32 { if y > 1 { right(y) } else { shared(y) } }",
        );
        let left = rows[0].similarity_features.as_ref().unwrap();
        let right = rows[1].similarity_features.as_ref().unwrap();
        let actual = BODY_WEIGHT * jaccard(&left.body_shingles, &right.body_shingles)
            + IDENTIFIER_WEIGHT * btree_jaccard(&left.identifiers, &right.identifiers)
            + SIGNATURE_WEIGHT * jaccard(&left.signature_shingles, &right.signature_shingles)
            + CONTROL_FLOW_WEIGHT * control_similarity(&left.control_flow, &right.control_flow);
        assert!(similarity_upper_bound(left, right) >= actual);
    }

    #[test]
    fn irrelevant_functions_do_not_inflate_exact_duplicate_groups() {
        let mut rows = function_rows("fn a(x: i32) -> i32 { x + 1 } fn b(y: i32) -> i32 { y + 2 }");
        rows[1].relevant_for_duplicates = false;
        assign_function_groups(&mut rows);
        assert_eq!(rows[0].duplicate_count, 1);
        assert_eq!(rows[1].duplicate_count, 0);
        assert!(rows.iter().all(|row| row.group_id.is_none()));
    }
}
