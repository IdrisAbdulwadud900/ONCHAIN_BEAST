// Telegram Bot (single-feature): side-wallet tracing with CEX-hop heuristics.
//
// Build: cargo build --release --bin telegram_bot
// Run:   TELEGRAM_BOT_TOKEN=... ./target/release/telegram_bot

use reqwest::Client;
use serde_json::Value;
use teloxide::prelude::*;
use teloxide::types::ParseMode;

fn api_base() -> String {
    std::env::var("ONCHAIN_BEAST_API_BASE").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn api_key() -> Option<String> {
    std::env::var("ONCHAIN_BEAST_API_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn looks_like_wallet(s: &str) -> bool {
    let s = s.trim();
    let len = s.len();
    (len == 44 || len == 32) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn short_addr(s: &str) -> String {
    let s = s.trim();
    if s.len() <= 10 {
        return s.to_string();
    }
    format!("{}...{}", &s[0..4], &s[s.len() - 4..])
}

fn http_client() -> Client {
    Client::builder()
        .no_proxy()
        .build()
        .expect("Failed to build reqwest client")
}

async fn get_json(url: &str) -> Result<Value, String> {
    let client = http_client();
    let mut req = client.get(url);
    if let Some(key) = api_key() {
        req = req.header("X-API-Key", key);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if !status.is_success() {
        return Err(format!("API error {}: {}", status.as_u16(), text));
    }

    serde_json::from_str(&text).map_err(|e| format!("Bad JSON: {} (raw={})", e, text))
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting OnChain Beast Telegram Bot (side-wallet tracing)");

    let bot_token = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!(
                "TELEGRAM_BOT_TOKEN is not set.\n\nRun:\n  TELEGRAM_BOT_TOKEN=... ./target/release/telegram_bot\n\nOptional:\n  ONCHAIN_BEAST_API_BASE=http://127.0.0.1:8080"
            );
            std::process::exit(1);
        }
    };

    // Teloxide's default client builder can try to read macOS system proxy settings,
    // which may panic in sandboxed environments. We disable proxy auto-detection.
    let telegram_client = teloxide::net::default_reqwest_settings()
        .no_proxy()
        .build()
        .expect("Failed to build teloxide reqwest client");
    let bot = Bot::with_client(bot_token, telegram_client);
    let handler = Update::filter_message().endpoint(handle_message);

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };

    let text = text.trim();
    let mut parts = text.splitn(2, ' ');
    let raw_cmd = parts.next().unwrap_or("");
    let cmd = raw_cmd.split('@').next().unwrap_or(raw_cmd);
    let arg = parts.next().unwrap_or("").trim();

    match cmd {
        "/start" | "/help" => {
            let help = "<b>OnChain Beast</b>\n\n\
<b>One feature:</b> Find likely side-wallets, including through CEX hops.\n\n\
<b>Commands</b>\n\
/track &lt;wallet&gt;  - trace side-wallets\n\n\
Tip: you can also paste a wallet address directly.";

            bot.send_message(msg.chat.id, help)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        "/track" => {
            if arg.is_empty() || !looks_like_wallet(arg) {
                bot.send_message(msg.chat.id, "Usage: /track <wallet_address>")
                    .await?;
                return Ok(());
            }
            track_wallet(&bot, msg.chat.id, arg).await?;
        }
        _ => {
            if looks_like_wallet(text) {
                track_wallet(&bot, msg.chat.id, text).await?;
            } else {
                bot.send_message(
                    msg.chat.id,
                    "Use /track <wallet> or paste a wallet. Send /help.",
                )
                .await?;
            }
        }
    }

    Ok(())
}

async fn track_wallet(bot: &Bot, chat_id: ChatId, wallet: &str) -> ResponseResult<()> {
    let url = format!(
        "{}/api/v1/wallet/{}/side-wallets?bootstrap=true&bootstrap_limit=25&depth=2&threshold=0.10&limit=10&lookback_days=30&cex_hops=true&cex_bootstrap_limit=15",
        api_base(),
        wallet
    );

    let v = match get_json(&url).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("❌ {}", e)).await?;
            return Ok(());
        }
    };

    let side_wallets = v
        .get("side_wallets")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let cex_wallets = v
        .get("cex_funded_wallets")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let mut lines = Vec::new();
    lines.push(format!(
        "<b>Trace</b> for <code>{}</code>",
        short_addr(wallet)
    ));

    if !side_wallets.is_empty() {
        lines.push("\n<b>Direct / Graph Candidates</b>".to_string());
        for (i, item) in side_wallets.iter().take(5).enumerate() {
            let addr = item.get("address").and_then(|x| x.as_str()).unwrap_or("?");
            let score = item.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0);
            lines.push(format!(
                "{}. <code>{}</code> (score {:.2})",
                i + 1,
                short_addr(addr),
                score
            ));
        }
    } else {
        lines.push(
            "\n<b>Direct / Graph Candidates</b>\nNone yet (try again after ingesting more txs)."
                .to_string(),
        );
    }

    if !cex_wallets.is_empty() {
        lines.push("\n<b>CEX-Hop Funded Wallets (Heuristic)</b>".to_string());
        for (i, item) in cex_wallets.iter().take(5).enumerate() {
            let addr = item.get("wallet").and_then(|x| x.as_str()).unwrap_or("?");
            let score = item.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0);

            let mut via = String::new();
            if let Some(paths) = item.get("paths").and_then(|x| x.as_array()) {
                if let Some(p) = paths.first() {
                    let dep = p
                        .get("deposit_wallet")
                        .and_then(|x| x.as_str())
                        .unwrap_or("?");
                    let hot = p.get("hot_wallet").and_then(|x| x.as_str()).unwrap_or("?");
                    via = format!(" via {}->{}", short_addr(dep), short_addr(hot));
                }
            }

            lines.push(format!(
                "{}. <code>{}</code> (score {:.2}){}",
                i + 1,
                short_addr(addr),
                score,
                via
            ));
        }
        lines.push("\n<i>Note: CEX hops are probabilistic (exchanges pool funds).</i>".to_string());
    }

    let msg = lines.join("\n");
    bot.send_message(chat_id, msg)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}
