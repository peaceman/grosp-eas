use futures::stream::{FuturesUnordered, StreamExt};
use std::path::{Path, PathBuf};
use tokio::fs::DirEntry;
use tracing::error;

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
        match join_handle_result {
            Ok(Ok(result)) => results.push(result),
            Ok(Err(e)) => error!("ParseError: {:?}", e),
            _ => (),
        }
    }

    Ok(results)
}

pub fn path_append(path: impl AsRef<Path>, append: &str) -> PathBuf {
    let mut os = path.as_ref().to_path_buf().into_os_string();
    os.push(append);

    PathBuf::from(os)
}
