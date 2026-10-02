//! The `--steam-worker` side: one Steamworks session per process.
use super::{Item, Request, Response, State};
use std::{
    collections::{BTreeSet, HashMap},
    io::{Read, Write},
    sync::mpsc,
    time::{Duration, Instant},
};
use steamworks::{Client, PublishedFileId, SteamId};

const APP_ID: u32 = 1_142_710;
/// Steam returns at most 50 items per details query.
const PAGE: usize = 50;
const WAIT: Duration = Duration::from_secs(60);
/// Persona names arrive asynchronously after `RequestUserInformation`.
const NAME_WAIT: Duration = Duration::from_millis(1500);

/// Entry point of `--steam-worker`: answer one request from stdin on stdout.
pub fn serve() -> std::process::ExitCode {
    let response = read_request()
        .map_err(|error| error.to_string())
        .and_then(|request| run(&request));
    let response = response.unwrap_or_else(Response::Failed);
    let bytes = serde_json::to_vec(&response).unwrap_or_default();
    let mut stdout = std::io::stdout();
    let written = stdout
        .write_all(super::RESPONSE_MARKER)
        .and_then(|()| stdout.write_all(&bytes))
        .and_then(|()| stdout.flush());
    if written.is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn read_request() -> crate::Result<Request> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| crate::error::io(std::path::Path::new("stdin"), e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn run(request: &Request) -> Result<Response, String> {
    let client = Client::init_app(APP_ID).map_err(|error| error.to_string())?;
    let ugc = client.ugc();
    Ok(match request {
        Request::Subscribed => Response::Subscribed(
            ugc.subscribed_items(true)
                .into_iter()
                .map(|id| id.0)
                .collect(),
        ),
        Request::State { ids } => {
            Response::States(ids.iter().map(|&id| state(&client, id)).collect())
        }
        Request::Details { ids } => Response::Details(details(&client, ids)?),
        Request::Download { ids } => {
            let (mut accepted, mut failed) = (Vec::new(), Vec::new());
            for &id in ids {
                if ugc.download_item(PublishedFileId(id), true) {
                    accepted.push(id);
                } else {
                    failed.push((id, "download was not accepted".into()));
                }
            }
            Response::Done { accepted, failed }
        }
        Request::Subscribe { ids } | Request::Unsubscribe { ids } => {
            let subscribe = matches!(request, Request::Subscribe { .. });
            let (sender, receiver) = mpsc::channel();
            for &id in ids {
                let sender = sender.clone();
                let done = move |result: Result<(), steamworks::SteamError>| {
                    let _ = sender.send((id, result.map_err(|error| error.to_string())));
                };
                if subscribe {
                    ugc.subscribe_item(PublishedFileId(id), done);
                } else {
                    ugc.unsubscribe_item(PublishedFileId(id), done);
                }
            }
            drop(sender);
            let (mut accepted, mut failed) = (Vec::new(), Vec::new());
            for (id, result) in pump(&client, receiver, ids.len()) {
                match result {
                    Ok(()) => accepted.push(id),
                    Err(error) => failed.push((id, error)),
                }
            }
            if subscribe {
                // Start the downloads right away, as the original does after subscribing.
                for &id in &accepted {
                    ugc.download_item(PublishedFileId(id), true);
                }
            }
            Response::Done { accepted, failed }
        }
    })
}

fn state(client: &Client, id: u64) -> State {
    let ugc = client.ugc();
    let item = PublishedFileId(id);
    let (downloaded, total) = ugc.item_download_info(item).unwrap_or_default();
    State {
        id,
        state: ugc.item_state(item).bits(),
        installed: ugc.item_install_info(item).map(|info| info.timestamp),
        downloaded,
        total,
    }
}

/// Run Steam callbacks until `expected` results arrive or the wait expires.
fn pump<T>(client: &Client, receiver: mpsc::Receiver<T>, expected: usize) -> Vec<T> {
    let started = Instant::now();
    let mut results = Vec::with_capacity(expected);
    while results.len() < expected && started.elapsed() < WAIT {
        client.run_callbacks();
        while let Ok(result) = receiver.try_recv() {
            results.push(result);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    results
}

fn details(client: &Client, ids: &[u64]) -> Result<Vec<Item>, String> {
    let ugc = client.ugc();
    let (sender, receiver) = mpsc::channel();
    let pages: Vec<_> = ids.chunks(PAGE).collect();
    for page in &pages {
        let query = ugc
            .query_items(page.iter().map(|&id| PublishedFileId(id)).collect())
            .map_err(|_| "could not create a Workshop query".to_owned())?
            .include_children(true)
            .allow_cached_response(0);
        let sender = sender.clone();
        query.fetch(move |result| {
            let page = result.map(|results| {
                (0..results.returned_results())
                    .filter_map(|index| {
                        let item = results.get(index)?;
                        let required = results
                            .get_children(index)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|id| id.0)
                            .collect();
                        Some((
                            item.owner.raw(),
                            Item {
                                id: item.published_file_id.0,
                                title: item.title,
                                author: String::new(),
                                file_name: item.file_name,
                                time_updated: item.time_updated,
                                tags: item.tags,
                                required,
                                banned: item.banned,
                            },
                        ))
                    })
                    .collect::<Vec<_>>()
            });
            let _ = sender.send(page.map_err(|error| error.to_string()));
        });
    }
    drop(sender);
    let mut items = Vec::with_capacity(ids.len());
    for page in pump(client, receiver, pages.len()) {
        items.extend(page?);
    }
    let owners: BTreeSet<u64> = items.iter().map(|(owner, _)| *owner).collect();
    let names = persona_names(client, &owners);
    Ok(items
        .into_iter()
        .map(|(owner, mut item)| {
            item.author = names.get(&owner).cloned().unwrap_or_default();
            item
        })
        .collect())
}

/// Ask Steam for creator names and give it a moment to deliver them, as the original does.
fn persona_names(client: &Client, owners: &BTreeSet<u64>) -> HashMap<u64, String> {
    let friends = client.friends();
    let pending = owners
        .iter()
        .filter(|&&owner| friends.request_user_information(SteamId::from_raw(owner), true))
        .count();
    if pending > 0 {
        let started = Instant::now();
        while started.elapsed() < NAME_WAIT {
            client.run_callbacks();
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    owners
        .iter()
        .map(|&owner| (owner, friends.get_friend(SteamId::from_raw(owner)).name()))
        .filter(|(_, name)| !name.is_empty() && name != "[unknown]")
        .collect()
}
