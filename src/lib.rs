#![allow(clippy::wildcard_imports)]
use anyhow::{Context, Result};
use reqwest::header::{self, HeaderMap};
use seed::{browser::web_storage::LocalStorage, prelude::*, *};
use serde::Deserialize;

#[derive(Clone)]
struct Form {
    organization: String,
    token: String,
}

#[derive(Debug)]
struct Organization {
    reviewers: Vec<Reviewer>,
    repositories: Vec<Repository>,
}

#[derive(Debug, Deserialize)]
struct Repository {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Reviewer {
    name: String,
    assigned_pull_requests: Vec<PullRequest>,
}

#[derive(Debug, Deserialize, Clone)]
struct PullRequest {
    id: String,
    url: String,
    repo_name: String,
}

struct Model {
    form: Form,
    organization: Option<Organization>,
    error_message: Option<String>,
    loading: bool,
}

enum Msg {
    Inputorganization(String),
    InputToken(String),
    SubmitClicked,
    LoadLocalStorage,
    FetchData,
    DataFetched(Result<Organization>),
    LoadingStarted,
    LoadingFinished,
}

fn init(_: Url, orders: &mut impl Orders<Msg>) -> Model {
    let model = Model {
        form: Form{organization: "".to_string(), token: "".to_string()},
        organization: None,
        error_message: None,
        loading: false,
    };
    orders.send_msg(Msg::LoadLocalStorage);
    model
}

#[wasm_bindgen(start)]
pub async fn start() {
    App::start("app", init, update, view);
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Inputorganization(organization) => {model.form.organization = organization}
        Msg::InputToken(token) => {model.form.token = token}
        Msg::SubmitClicked => {
            LocalStorage::insert("organization", &model.form.organization).unwrap_or_default();
            LocalStorage::insert("token", &model.form.token).unwrap_or_default();
            orders.send_msg(Msg::FetchData);
        }
        Msg::LoadLocalStorage => {
            model.form.organization = LocalStorage::get("organization").unwrap_or_default();
            model.form.token = LocalStorage::get("token").unwrap_or_default();
        }
        Msg::FetchData => {
            orders.send_msg(Msg::LoadingStarted);
            orders.perform_cmd(fetch_organization_data(model.form.clone()).map(Msg::DataFetched));
        }
        Msg::DataFetched(result) => {
            orders.send_msg(Msg::LoadingFinished);
            match result {
                Ok(organization) => model.organization = Some(organization),
                Err(err) => model.error_message = Some(err.to_string()),
            }
        }
        Msg::LoadingStarted => {
            model.loading = true;
        }
        Msg::LoadingFinished => {
            model.loading = false;
        }
    }
}

async fn fetch_organization_data(form: Form) -> Result<Organization> {
    let organization = form.organization;
    let token = form.token;
    let mut org = Organization {reviewers: vec![], repositories: vec![]};
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );
    headers.insert(header::USER_AGENT, "ibr".parse().unwrap());
    // セッションを再利用して複数回リクエストするためのインスタンスを生成する
    let client = reqwest::Client::new();
    let repositories_url = format!("https://api.github.com/orgs/{}/repos", organization);
    let repositories_response = &client
        .get(&repositories_url)
        .headers(headers.clone())
        .send()
        .await
        .with_context(|| format!("Failed to fetch repositories from {}", repositories_url))?
        .text()
        .await
        .with_context(|| "Failed to parse repositories response")?;
    let mut repositories: Vec<Repository> =
        serde_json::from_str(&repositories_response).unwrap_or_else(|_| Vec::new());
    for repository in &mut repositories {
        let pulls_url = format!(
            "https://api.github.com/repos/{}/{}/pulls?state=open",
            organization, repository.name
        );
        let pulls_response = &client
            .get(&pulls_url)
            .headers(headers.clone())
            .send()
            .await
            .with_context(|| format!("Failed to fetch pull requests from {}", pulls_url))?
            .text()
            .await
            .with_context(|| "Failed to parse pull requests response")?;
        let pulls: Vec<serde_json::Value> = serde_json::from_str(&pulls_response)
            .with_context(|| "Failed to parse pull requests")?;

        for pull in pulls {
            if !org
                .repositories
                .iter()
                .any(|repo| repo.name == repository.name)
            {
                org.repositories.push(Repository {
                    name: repository.name.to_string(),
                });
            };

            let reviewers = serde_json::Value::as_array(&pull["requested_reviewers"]).unwrap();
            for reviewer in reviewers {
                let reviewer_name = reviewer["login"].clone();

                if !org
                    .reviewers
                    .iter()
                    .any(|r| r.name.to_string() == reviewer["login"].to_string())
                {
                    org.reviewers.push(Reviewer {
                        name: reviewer_name.to_string(),
                        assigned_pull_requests: vec![],
                    });
                };
                let _index = org
                    .reviewers
                    .iter()
                    .position(|r| r.name == reviewer_name.to_string());

                if !_index.is_none() {
                    let index = _index.unwrap();

                    org.reviewers[index]
                        .assigned_pull_requests
                        .push(PullRequest {
                            id: pull["number"].to_string(),
                            url: pull["url"]
                                .as_str()
                                .unwrap()
                                .replace("api.", "")
                                .replace("repos/", "")
                                .replace("pulls", "pull"),
                            repo_name: repository.name.to_string(),
                        });
                }
            }
        }
    }

    Ok(org)
}

fn view(model: &Model) -> Node<Msg> {
    div![
        h1![a![
            attrs! {
                At::Href => "https://github.com/yoshiichn/IBR",
                At::Target => "_blank",
                At::Rel => "noopener noreferrer",
            },
            format!("{} I'm Busy Reviewing. {}", '\u{1F347}', '\u{1F980}')
        ]],
        form![
            input![
                attrs! {
                    At::Type => "text",
                    At::Value => &model.form.organization,
                },
                input_ev(Ev::Input, Msg::Inputorganization),
            ],
            input![
                attrs! {
                    At::Type => "text",
                    At::Value => &model.form.token,
                },
                input_ev(Ev::Input, Msg::InputToken),
            ],
            button![
                "Submit",
                ev(Ev::Click, |_| Msg::SubmitClicked),
            ],
        ],
        button![
            "Fetch data",
            ev(Ev::Click, |_| Msg::FetchData),
            style![
                St::BackgroundColor => "#2c3e50",
                St::Color => "#ffffff",
                St::Padding => "10px 20px",
                St::BorderRadius => "5px",
                St::Cursor => "pointer",
            ],
        ],
        div![if model.loading {
            loading_spinner()
        } else {
            empty![]
        }],
        match &model.organization {
            Some(organization) => {
                div![
                    table![
                        style![
                            St::BorderCollapse => "collapse",
                            St::Width => "100%",
                            St::MarginBottom => "20px",
                        ],
                        thead![
                            style![
                                St::BackgroundColor => "#2c3e50",
                                St::Color => "#ffffff",
                            ],
                            tr![
                                style![
                                    St::FontWeight => "bold",
                                    St::Padding => "10px",
                                    St::TextAlign => "left",
                                ],
                                th!["Users"],
                                organization.repositories.iter().map(|repo| {
                                    th![
                                        style![
                                            St::Padding => "10px",
                                            St::TextAlign => "center",
                                        ],
                                        &repo.name
                                    ]
                                })
                            ]
                        ],
                        tbody![organization.reviewers.iter().map(|reviewer| {
                            tr![
                                td![
                                    img![
                                        attrs! {
                                            At::Src => format!("https://github.com/{}.png", reviewer.name.chars().filter(|&c| c != '\"').collect::<String>()),
                                            At::Alt => &reviewer.name,
                                            At::Width => "40",
                                            At::Height => "40",
                                        }
                                    ],
                                    style![
                                        St::Padding => "10px",
                                        St::VerticalAlign => "baseline",
                                    ],
                                    a![ reviewer.name.chars().filter(|&c| c != '\"').collect::<String>()]
                                ],
                                organization.repositories.iter().map(|repo| {
                                    let prs: Vec<PullRequest> = reviewer
                                        .assigned_pull_requests
                                        .iter()
                                        .filter(|pr| pr.repo_name == *repo.name)
                                        .cloned()
                                        .collect();
                                    td![
                                        style![
                                            St::Padding => "10px",
                                            St::VerticalAlign => "top",
                                            St::TextAlign => "center",
                                        ],
                                        prs.iter().map(|pr| {
                                            a![
                                                style![
                                                    St::BoxShadow => "0px 0px 5px 2px rgba(0,0,0,0.5)",
                                                    St::BackgroundColor => "white",
                                                    St::Color => "#ffffff",
                                                    St::TextDecoration => "none",
                                                    St::Padding => "5px 10px",
                                                    St::BorderRadius => "5px",
                                                    St::Cursor => "pointer",
                                                ],
                                                a![
                                                    attrs! {
                                                        At::Href => &pr.url,
                                                        At::Target => "_blank",
                                                        At::Rel => "noopener noreferrer",
                                                    },
                                                    &pr.id
                                                ]
                                            ]
                                        })
                                    ]
                                })
                            ]
                        })]
                    ]
                ]
            }
            None => match &model.error_message {
                Some(error_message) => p![
                    style![
                        St::FontWeight => "bold",
                        St::Color => "red",
                    ],
                    error_message
                ],
                None => empty![],
            },
        }
    ]
}

fn loading_spinner() -> Node<Msg> {
    div![style![
        St::Display => "inline-block",
        St::Width => "1.5rem",
        St::Height => "1.5rem",
        St::BorderRadius => "50%",
        St::BorderStyle => "solid",
        St::BorderWidth => "0.2rem",
        St::BorderColor => "#eee #eee #eee #007bff",
        St::Position => "absolute",
        St::Top => "50%",
        St::Left => "50%",
        St::Transform => "translate(-50%, -50%)",
        St::Animation => "spin 1s linear infinite"
    ],]
}
