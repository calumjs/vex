use std::path::PathBuf;

use anyhow::Result;
use ignore::WalkBuilder;

use crate::Cli;

/// Walk the specified paths and return all matching file paths.
pub fn walk_paths(cli: &Cli) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for root in &cli.paths {
        let mut builder = WalkBuilder::new(root);
        builder.hidden(!cli.hidden);
        builder.git_ignore(!cli.no_gitignore);
        builder.git_global(!cli.no_gitignore);
        builder.git_exclude(!cli.no_gitignore);

        // Add custom .vexignore support
        builder.add_custom_ignore_filename(".vexignore");

        // Apply glob filter if specified
        if let Some(ref pattern) = cli.glob {
            let mut types_builder = ignore::types::TypesBuilder::new();
            types_builder.add("custom", pattern)?;
            types_builder.select("custom");
            builder.types(types_builder.build()?);
        }

        let walker = builder.build();
        for entry in walker {
            let entry = entry?;
            if entry.file_type().is_some_and(|ft| ft.is_file()) {
                files.push(entry.into_path());
            }
        }
    }

    files.sort();
    Ok(files)
}
