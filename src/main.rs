use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::env;
use reqwest::Client;
use dotenvy::dotenv;

const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

#[derive(Deserialize, Debug)]
struct UserRequest {
    messages: Option<Vec<Message>>,
    message: Option<String>,
    image_url: Option<String>,
    image_base64: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    role: String,
    content: serde_json::Value, // Use serde_json::Value to handle both string and array
}

#[derive(Serialize)]
struct ApiPayload {
    model: String,
    messages: Vec<Message>,
    temperature: f64,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatResponse {
    reply: String,
}

#[derive(Deserialize)]
struct GroqChoice {
    message: GroqMessage,
}

#[derive(Deserialize)]
struct GroqMessage {
    content: String,
}

#[derive(Deserialize)]
struct GroqResponse {
    choices: Vec<GroqChoice>,
}

#[get("/")]
async fn home() -> impl Responder {
    HttpResponse::Ok().body("🧠 Groq Unified Chat + Vision API is running!")
}

#[post("/chat")]
async fn chat(req: web::Json<UserRequest>) -> HttpResponse {
    dotenv().ok();
    let api_key = match env::var("GROQ_API_KEY") {
        Ok(key) => key,
        Err(_) => return HttpResponse::InternalServerError().json("API key not found"),
    };

    let client = Client::new();

    // Determine the API payload based on the request
    let payload = if let Some(message) = &req.message {
        // Handle Vision or Single-turn text
        if req.image_url.is_some() || req.image_base64.is_some() {
            let image_data = if let Some(url) = &req.image_url {
                serde_json::json!({"url": url})
            } else {
                serde_json::json!({"url": req.image_base64})
            };
            
            let content = serde_json::json!([
                {"type": "text", "text": message},
                {"type": "image_url", "image_url": image_data}
            ]);

            let messages = vec![Message {
                role: "user".to_string(),
                content: content,
            }];

            ApiPayload {
                model: "meta-llama/llama-4-scout-17b-16e-instruct".to_string(),
                messages,
                temperature: 0.5,
                max_tokens: 1024,
            }
        } else {
            // Single-turn text
            let messages = vec![Message {
                role: "user".to_string(),
                content: serde_json::Value::String(message.clone()),
            }];

            ApiPayload {
                model: "deepseek-r1-distill-llama-70b".to_string(),
                messages,
                temperature: 0.5,
                max_tokens: 1024,
            }
        }
    } else if let Some(messages) = &req.messages {
        // Multi-turn text
        ApiPayload {
            model: "deepseek-r1-distill-llama-70b".to_string(),
            messages: messages.clone(),
            temperature: 0.5,
            max_tokens: 1024,
        }
    } else {
        return HttpResponse::BadRequest().json("No valid input provided");
    };

    // Make the API call
    let res = match client.post(GROQ_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await {
            Ok(r) => r,
            Err(e) => {
                let error_message = format!("Groq API failed: {}", e);
                return HttpResponse::InternalServerError().json(error_message);
            }
        };

    // Parse and return the response
    if res.status().is_success() {
        match res.json::<GroqResponse>().await {
            Ok(groq_res) => {
                if let Some(choice) = groq_res.choices.get(0) {
                    HttpResponse::Ok().json(ChatResponse {
                        reply: choice.message.content.trim().to_string(),
                    })
                } else {
                    HttpResponse::InternalServerError().json("No choices found in API response")
                }
            }
            Err(_) => HttpResponse::InternalServerError().json("Failed to parse API response"),
        }
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        let error_message = format!("Groq API returned an error: {} - {}", status, body);
        HttpResponse::Status(status).json(error_message)
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .service(home)
            .service(chat)
    })
    .bind(("0.0.0.0", 10000))?
    .run()
    .await
}
