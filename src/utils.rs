use futures::stream::{FuturesUnordered, StreamExt};
use std::path::{Path, PathBuf};
use tokio::fs::DirEntry;

pub async fn scan_for_files(path: impl AsRef<Path>) -> anyhow::Result<Vec<DirEntry>> {
    let dir_entries = tokio::fs::read_dir(path).await?;

    let files_in_folder = dir_entries
        .filter_map(|e| async {
            if let Ok(dir_entry) = e {
                dir_entry
                    .file_type()
                    .await
                    .ok()
                    .filter(|file_type| file_type.is_file())
                    .map(|_| dir_entry)
            } else {
                None
            }
        })
        .collect()
        .await;

    Ok(files_in_folder)
}

pub async fn parse_files<T: 'static + Send>(
    files: Vec<DirEntry>,
    parse_fn: impl Fn(PathBuf) -> anyhow::Result<T> + 'static + Send + Copy,
) -> anyhow::Result<Vec<T>> {
    let mut handles = files
        .iter()
        .map(|file| {
            let path = file.path();
            tokio::task::spawn_blocking(move || parse_fn(path))
        })
        .collect::<FuturesUnordered<_>>();

    let mut results = vec![];

    while let Some(join_handle_result) = handles.next().await {
        if let Ok(Ok(result)) = join_handle_result {
            results.push(result);
        }
    }

    Ok(results)
}