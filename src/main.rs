use axum::{
    routing::{get, post},
    Json, Router,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use dotenvy::dotenv;
use reqwest::Client;
use serde_json::json;
use hyper::server::Server;// ✅ axum v0.7 के लिए जरूरी

#[derive(Clone)]
struct AppState {
    api_key: String,
    client: Client,
}

#[derive(Deserialize)]
struct PromptRequest {
    prompt: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let api_key = env::var("GROQ_API_KEY").expect("Missing GROQ_API_KEY");
    let client = Client::new();

    let app_state = Arc::new(AppState { api_key, client });

    let app = Router::new()
        .route("/", get(home))
        .route("/chat", post(chat))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 10000));
    println!("🧠 Rust Excel AI API running on {}", addr);

    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn home() -> &'static str {
    "🧠 Groq Excel AI API is Running!"
}

async fn chat(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PromptRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if body.prompt.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No prompt provided".to_string(),
            }),
        ));
    }

    let system_msg = "You are an intelligent Excel assistant AI. The user will describe tasks in natural language, like: 'Write Total in A1', 'Set A2 to 500', 'Highlight B3', or 'Fill A1 to A5 with names'. Your job is to convert this into a list of actions in JSON format like: [{\"cell\": \"A1\", \"value\": \"Total\", \"highlight\": true}, {\"cell\": \"A2\", \"value\": \"500\"}, {\"cell\": \"A3\", \"value\": \"=A1+A2\"}]. Only return the JSON list. No explanation, no markdown. Only pure JSON array.";

    let payload = json!({
        "model": "llama3-8b-8192",
        "messages": [
            { "role": "system", "content": system_msg },
            { "role": "user", "content": body.prompt }
        ]
    });

    let res = state
        .client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(&state.api_key)
        .json(&payload)
        .send()
        .await;

    match res {
        Ok(response) => {
            let status = response.status();
            let json_data = response.json::<serde_json::Value>().await.unwrap_or_else(|_| {
                json!({"error": "Invalid JSON from Groq"})
            });

            if status.is_success() {
                let reply = json_data["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .trim();

                match serde_json::from_str::<serde_json::Value>(reply) {
                    Ok(actions) => Ok(Json(json!({ "actions": actions }))),
                    Err(_) => Ok(Json(json!({ "response": reply }))),
                }
            } else {
                Err((
                    status,
                    Json(ErrorResponse {
                        error: "Groq API failed".to_string(),
                    }),
                ))
            }
        }
        Err(_) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: "Groq API timeout".to_string(),
            }),
        )),
    }
}