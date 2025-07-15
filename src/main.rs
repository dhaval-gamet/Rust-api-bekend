use axum::{
    routing::{get, post},
    Json, Router,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use dotenvy::dotenv; // Keep dotenvy as it's a valid alternative
use reqwest::Client;
use serde_json::json;
use axum::serve; // ✅ Axum v0.7+ के लिए सही तरीका

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
    let api_key = env::var("GROQ_API_KEY").expect("Missing GROQ_API_KEY environment variable. Please set it.");
    let client = Client::new();

    let app_state = Arc::new(AppState { api_key, client });

    let app = Router::new()
        .route("/", get(home))
        .route("/chat", post(chat))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 10000));
    println!("🧠 Rust Excel AI API running on {}", addr);

    // Axum v0.7+ में इस तरह से सर्वर को चलाएं
    serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app.into_make_service())
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
            
            if status.is_success() {
                let json_data = response.json::<serde_json::Value>().await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to parse Groq API response JSON: {}", e),
                        }),
                    )
                })?;

                let reply = json_data["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .trim();

                // यहाँ हम यह सुनिश्चित करने की कोशिश करते हैं कि प्रतिक्रिया एक वैध JSON ऐरे है
                match serde_json::from_str::<serde_json::Value>(reply) {
                    Ok(actions) => {
                        // यहाँ अतिरिक्त जाँच जोड़ सकते हैं कि actions एक ऐरे है या नहीं
                        if actions.is_array() {
                            Ok(Json(json!({ "actions": actions })))
                        } else {
                            // अगर यह JSON है लेकिन ऐरे नहीं है, तो त्रुटि दें
                            Err((
                                StatusCode::UNPROCESSABLE_ENTITY, // या कोई अन्य उपयुक्त स्थिति कोड
                                Json(ErrorResponse {
                                    error: format!("Groq API returned valid JSON but not an array as expected: {}", reply),
                                }),
                            ))
                        }
                    },
                    Err(_) => {
                        // अगर Groq का जवाब JSON नहीं है, तो त्रुटि दें
                        Err((
                            StatusCode::UNPROCESSABLE_ENTITY, // या कोई अन्य उपयुक्त स्थिति कोड
                            Json(ErrorResponse {
                                error: format!("Groq API returned invalid JSON: {}", reply),
                            }),
                        ))
                    },
                }
            } else {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error from Groq API".to_string());
                Err((
                    status,
                    Json(ErrorResponse {
                        error: format!("Groq API failed with status {}: {}", status, error_text),
                    }),
                ))
            }
        }
        Err(e) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: format!("Groq API request failed: {}", e),
            }),
        )),
    }
}
