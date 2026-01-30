// Telegram Bot for OnChain Beast - Simplified version
// Build with: cargo build --release --bin telegram_bot
// Run with: TELEGRAM_BOT_TOKEN=your_token ./target/release/telegram_bot

use reqwest::Client;
use teloxide::prelude::*;

fn api_base() -> String {
    std::env::var("ONCHAIN_BEAST_API_BASE").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn api_key() -> Option<String> {
    std::env::var("ONCHAIN_BEAST_API_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("🤖 Starting OnChain Beast Telegram Bot");

    let bot_token = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("❌ TELEGRAM_BOT_TOKEN is not set.\n\nRun:\n  TELEGRAM_BOT_TOKEN=... ./target/release/telegram_bot\n\nOptional:\n  ONCHAIN_BEAST_API_BASE=http://127.0.0.1:8080");
            std::process::exit(1);
        }
    };

    let bot = Bot::new(bot_token);

    let handler = Update::filter_message().endpoint(handle_message);

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    if let Some(text) = msg.text() {
        let text = text.trim();
        let mut parts = text.splitn(2, ' ');
        let raw_cmd = parts.next().unwrap_or("");
        let cmd = raw_cmd.split('@').next().unwrap_or(raw_cmd);
        let arg = parts.next().unwrap_or("").trim();

        match cmd {
            "/start" | "/help" => {
                let help = "🤖 <b>OnChain Beast Wallet Analyzer</b>\n━━━━━━━━━━━━━━━━\n\n\
                    <b>Commands:</b>\n\
                    /analyze &lt;wallet&gt; - Analyze wallet\n\
                    /stats &lt;wallet&gt; - Quick stats\n\
                    /sidewallets &lt;wallet&gt; - Find side-wallet candidates\n\
                    /cluster &lt;wallet&gt; - Cluster summary (includes primary)\n\
                    /risk &lt;wallet&gt; - Risk score\n\
                    /patterns &lt;wallet&gt; - Pattern report\n\
                    /highrisk - High-risk wallets\n\
                    /token &lt;mint&gt; - Token info\n\
                    /status - API status\n\
                    /health - Service status\n\n\
                    <b>Quick Tip:</b>\n\
                    Just paste a wallet address to analyze!";

                bot.send_message(msg.chat.id, help)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
                return Ok(());
            }
            "/analyze" => {
                if arg.is_empty() {
                    bot.send_message(msg.chat.id, "❌ Usage: /analyze <wallet_address>")
                        .await?;
                    return Ok(());
                }
                analyze_wallet(&bot, msg.chat.id, arg).await?;
                return Ok(());
            }
            "/stats" => {
                if arg.is_empty() {
                    bot.send_message(msg.chat.id, "❌ Usage: /stats <wallet_address>")
                        .await?;
                    return Ok(());
                }
                get_wallet_stats(&bot, msg.chat.id, arg).await?;
                return Ok(());
            }
            "/cluster" => {
                if arg.is_empty() {
                    bot.send_message(msg.chat.id, "❌ Usage: /cluster <wallet_address>")
                        .await?;
                    return Ok(());
                }
                get_wallet_cluster(&bot, msg.chat.id, arg).await?;
                return Ok(());
            }
            "/sidewallets" => {
                if arg.is_empty() {
                    bot.send_message(msg.chat.id, "❌ Usage: /sidewallets <wallet_address>")
                        .await?;
                    return Ok(());
                }
                get_side_wallets(&bot, msg.chat.id, arg).await?;
                return Ok(());
            }
            "/risk" => {
                if arg.is_empty() {
                    bot.send_message(msg.chat.id, "❌ Usage: /risk <wallet_address>")
                        .await?;
                    return Ok(());
                }
                get_wallet_risk(&bot, msg.chat.id, arg).await?;
                return Ok(());
            }
            "/patterns" => {
                if arg.is_empty() {
                    bot.send_message(msg.chat.id, "❌ Usage: /patterns <wallet_address>")
                        .await?;
                    return Ok(());
                }
                get_wallet_patterns(&bot, msg.chat.id, arg).await?;
                return Ok(());
            }
            "/highrisk" => {
                get_high_risk_wallets(&bot, msg.chat.id).await?;
                return Ok(());
            }
            "/token" => {
                if arg.is_empty() {
                    bot.send_message(msg.chat.id, "❌ Usage: /token <mint_address>")
                        .await?;
                    return Ok(());
                }
                get_token_info(&bot, msg.chat.id, arg).await?;
                return Ok(());
            }
            "/status" => {
                get_status(&bot, msg.chat.id).await?;
                return Ok(());
            }
            "/health" => {
                check_health(&bot, msg.chat.id).await?;
                return Ok(());
            }
            _ => {}
        }

        // If 44 chars, treat as wallet address
        if text.len() == 44 && text.chars().all(|c| c.is_alphanumeric()) {
            analyze_wallet(&bot, msg.chat.id, text).await?;
            return Ok(());
        }

        // Default: show help
        bot.send_message(
            msg.chat.id,
            "👋 Send a wallet address or use /help for commands",
        )
        .await?;
    }

    Ok(())
}

async fn analyze_wallet(bot: &Bot, chat_id: ChatId, wallet: &str) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/analysis/wallet/{}", api_base(), wallet);

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let text = format_analysis(&data);
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ Invalid wallet address or no data")
                    .await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Service unavailable. Is the API running?")
                .await?;
        }
    }

    Ok(())
}

async fn get_wallet_stats(bot: &Bot, chat_id: ChatId, wallet: &str) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/transfer/wallet-stats/{}", api_base(), wallet);

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let text = format_stats(&data);
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ No data found for wallet")
                    .await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

async fn get_wallet_cluster(bot: &Bot, chat_id: ChatId, wallet: &str) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();

    // bootstrap=true so first-time users get data without manual ingestion.
    let url = format!(
        "{}/api/v1/wallet/{}/cluster?bootstrap=true&bootstrap_limit=25&depth=2&threshold=0.10&limit=20&lookback_days=30",
        api_base(),
        wallet
    );

    let mut req = client.get(&url);
    if let Some(key) = api_key() {
        req = req.header("X-API-Key", key);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.as_u16() == 401 {
                bot.send_message(
                    chat_id,
                    "🔒 This endpoint is protected. Set ONCHAIN_BEAST_API_KEY (and enable_auth on the API) or disable auth for local use.",
                )
                .await?;
                return Ok(());
            }
            match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    let text = format_wallet_cluster(wallet, &data);
                    bot.send_message(chat_id, text)
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .await?;
                }
                Err(_) => {
                    bot.send_message(chat_id, "❌ Cluster unavailable").await?;
                }
            }
        }
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

async fn get_side_wallets(bot: &Bot, chat_id: ChatId, wallet: &str) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    // bootstrap=true so first-time users get data without manual ingestion.
    let url = format!(
        "{}/api/v1/wallet/{}/side-wallets?bootstrap=true&bootstrap_limit=25&depth=2&threshold=0.10&limit=12&lookback_days=30",
        api_base(),
        wallet
    );

    let mut req = client.get(&url);
    if let Some(key) = api_key() {
        req = req.header("X-API-Key", key);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.as_u16() == 401 {
                bot.send_message(
                    chat_id,
                    "🔒 This endpoint is protected. Set ONCHAIN_BEAST_API_KEY (and enable_auth on the API) or disable auth for local use.",
                )
                .await?;
                return Ok(());
            }
            match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    let text = format_side_wallets(wallet, &data);
                    bot.send_message(chat_id, text)
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .await?;
                }
                Err(_) => {
                    bot.send_message(chat_id, "❌ Side-wallets unavailable").await?;
                }
            }
        }
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

async fn get_token_info(bot: &Bot, chat_id: ChatId, mint: &str) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/metadata/token/{}", api_base(), mint);

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let text = format_token(&data);
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ Token not found").await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

async fn check_health(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/health", api_base());

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let status = if let Some(s) = data.get("status").and_then(|s| s.as_str()) {
                    if s == "healthy" {
                        "✅ HEALTHY"
                    } else {
                        "⚠️ DEGRADED"
                    }
                } else {
                    "❌ UNKNOWN"
                };

                let text = format!(
                        "📊 <b>Service Status</b>\n━━━━━━━━━━━━━━━━\n\n{}\n\nAPI: http://127.0.0.1:8080",
                        status
                    );

                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ Service is down").await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Cannot reach service at http://127.0.0.1:8080")
                .await?;
        }
    }

    Ok(())
}

async fn get_status(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/status", api_base());

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let text = format_status(&data);
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ Status unavailable").await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

async fn get_high_risk_wallets(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/analysis/high-risk-wallets", api_base());

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let text = format_high_risk(&data);
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ No high-risk wallets found")
                    .await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

async fn get_wallet_patterns(bot: &Bot, chat_id: ChatId, wallet: &str) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/analysis/patterns/{}", api_base(), wallet);

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let text = format_patterns(wallet, &data);
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ No pattern data found")
                    .await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

async fn get_wallet_risk(bot: &Bot, chat_id: ChatId, wallet: &str) -> ResponseResult<()> {
    bot.send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
        .await?;

    let client = Client::new();
    let url = format!("{}/analysis/wallet/{}/risk-score", api_base(), wallet);

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let text = format_risk(&data);
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
            }
            Err(_) => {
                bot.send_message(chat_id, "❌ Risk score unavailable")
                    .await?;
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Service error").await?;
        }
    }

    Ok(())
}

fn format_analysis(data: &serde_json::Value) -> String {
    let wallet = data
        .get("wallet_address")
        .and_then(|a| a.as_str())
        .or_else(|| data.get("wallet").and_then(|a| a.as_str()))
        .unwrap_or("Unknown");
    let risk = data
        .get("risk_level")
        .and_then(|r| r.as_str())
        .unwrap_or("Unknown");
    let confidence = data
        .get("confidence_score")
        .and_then(|c| c.as_f64())
        .unwrap_or(0.0);
    let tx_count = data
        .get("transaction_count")
        .and_then(|c| c.as_u64())
        .unwrap_or(0);
    let patterns = data
        .get("patterns_detected")
        .and_then(|p| p.as_u64())
        .unwrap_or(0);
    let red_flags = data
        .get("red_flags")
        .and_then(|r| r.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let sol_in = data.get("total_sol_in").and_then(|s| s.as_f64());
    let sol_out = data.get("total_sol_out").and_then(|s| s.as_f64());

    let risk_emoji = match risk.to_lowercase().as_str() {
        "low" => "🟢",
        "medium" => "🟡",
        "high" => "🔴",
        _ => "⚪",
    };

    let mut text = format!(
        "📊 <b>Wallet Analysis</b>\n━━━━━━━━━━━━━━━━\n\n\
        <b>Address:</b>\n<code>{}</code>\n\n\
        <b>Risk:</b> {} {}\n\
        <b>Confidence:</b> {:.0}%\n\
        <b>Transactions:</b> {}\n\
        <b>Patterns Detected:</b> {}\n\
        <b>Red Flags:</b> {}",
        wallet,
        risk_emoji,
        risk.to_uppercase(),
        confidence * 100.0,
        tx_count,
        patterns,
        red_flags
    );

    if sol_in.is_some() || sol_out.is_some() {
        let sol_in = sol_in.unwrap_or(0.0);
        let sol_out = sol_out.unwrap_or(0.0);
        text.push_str(&format!(
            "\n\n💰 <b>Activity:</b>\nSOL In: {:.2}\nSOL Out: {:.2}",
            sol_in, sol_out
        ));
    }

    text
}

fn format_stats(data: &serde_json::Value) -> String {
    let wallet = data
        .get("wallet")
        .and_then(|a| a.as_str())
        .or_else(|| data.get("wallet_address").and_then(|a| a.as_str()))
        .unwrap_or("Unknown");
    let sol_transferred = data
        .get("total_sol_transferred")
        .and_then(|s| s.as_f64())
        .unwrap_or(0.0);
    let token_transfers = data
        .get("total_token_transfers")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    format!(
        "📈 <b>Wallet Stats</b>\n━━━━━━━━━━━━━━━━\n\n\
        <b>Address:</b>\n<code>{}</code>\n\n\
        <b>SOL Transferred:</b> {:.2}\n\
        <b>Token Transfers:</b> {}",
        wallet, sol_transferred, token_transfers
    )
}

fn format_high_risk(data: &serde_json::Value) -> String {
    let mut text = "🚨 <b>High-Risk Wallets</b>\n━━━━━━━━━━━━━━━━\n\n".to_string();

    if let Some(wallets) = data.get("wallets").and_then(|w| w.as_array()) {
        if wallets.is_empty() {
            text.push_str("No high-risk wallets returned.");
            return text;
        }

        for (idx, item) in wallets.iter().take(10).enumerate() {
            if let Some(pair) = item.as_array() {
                let addr = pair.get(0).and_then(|v| v.as_str()).unwrap_or("Unknown");
                let score = pair.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                text.push_str(&format!(
                    "{}. <code>{}</code>\n   Risk Score: {:.2}\n\n",
                    idx + 1,
                    addr,
                    score
                ));
            }
        }
    } else {
        text.push_str("No high-risk wallets returned.");
    }

    text
}

fn format_patterns(wallet: &str, data: &serde_json::Value) -> String {
    let wash = data
        .get("wash_trading")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let pump = data
        .get("pump_dump")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let circular = data
        .get("circular_flows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let coordinated = data
        .get("coordinated_activity")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    format!(
        "🧩 <b>Pattern Report</b>\n━━━━━━━━━━━━━━━━\n\n\
        <b>Address:</b>\n<code>{}</code>\n\n\
        Wash Trading: {}\n\
        Pump & Dump: {}\n\
        Circular Flows: {}\n\
        Coordinated Activity: {}",
        wallet, wash, pump, circular, coordinated
    )
}

fn format_risk(data: &serde_json::Value) -> String {
    let wallet = data
        .get("wallet")
        .and_then(|a| a.as_str())
        .unwrap_or("Unknown");
    let risk_score = data
        .get("risk_score")
        .and_then(|r| r.as_f64())
        .unwrap_or(0.0);
    let risk_level = data
        .get("risk_level")
        .and_then(|r| r.as_str())
        .unwrap_or("Unknown");
    let confidence = data
        .get("confidence")
        .and_then(|c| c.as_f64())
        .unwrap_or(0.0);

    format!(
        "🛡️ <b>Risk Score</b>\n━━━━━━━━━━━━━━━━\n\n\
        <b>Address:</b>\n<code>{}</code>\n\n\
        Risk Score: {:.2}\n\
        Risk Level: {}\n\
        Confidence: {:.0}%",
        wallet,
        risk_score,
        risk_level,
        confidence * 100.0
    )
}

fn format_status(data: &serde_json::Value) -> String {
    let status = data
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let nodes = data
        .get("cluster")
        .and_then(|c| c.get("nodes"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    format!(
        "📡 <b>API Status</b>\n━━━━━━━━━━━━━━━━\n\n\
        Status: {}\n\
        Cluster Nodes: {}",
        status.to_uppercase(),
        nodes
    )
}

fn format_token(data: &serde_json::Value) -> String {
    let symbol = data
        .get("symbol")
        .and_then(|s| s.as_str())
        .unwrap_or("Unknown");
    let name = data
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown");
    let decimals = data.get("decimals").and_then(|d| d.as_u64()).unwrap_or(0);

    format!(
        "🪙 <b>Token Info</b>\n━━━━━━━━━━━━━━━━\n\n\
        <b>Symbol:</b> {}\n\
        <b>Name:</b> {}\n\
        <b>Decimals:</b> {}",
        symbol, name, decimals
    )
}

fn format_side_wallets(wallet: &str, data: &serde_json::Value) -> String {
    let ingested = data
        .get("bootstrap_ingested_transactions")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let depth = data
        .get("analysis_depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let threshold = data
        .get("confidence_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let lookback_days = data
        .get("lookback_days")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    let mut text = format!(
        "🕵️ <b>Side-Wallet Candidates</b>\n━━━━━━━━━━━━━━━━\n\n\
<b>Primary:</b>\n<code>{}</code>\n\n\
Depth: {} | Threshold: {:.2} | Lookback: {}d\nBootstrap ingested: {} tx\n\n",
        wallet, depth, threshold, lookback_days, ingested
    );

    let Some(arr) = data.get("side_wallets").and_then(|v| v.as_array()) else {
        text.push_str("No results yet.");
        return text;
    };

    if arr.is_empty() {
        text.push_str(
            "No candidates found yet. Try again (more data may be ingested on the next run).\n",
        );
        return text;
    }

    for (i, item) in arr.iter().take(10).enumerate() {
        let addr = item
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let d = item.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
        let dir = item
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let shared_funders = item
            .get("shared_funders")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let shared_counterparties = item
            .get("shared_counterparties")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let last_seen = item
            .get("last_seen_epoch")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let last_seen_days = if last_seen > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now > last_seen {
                ((now - last_seen) as f64 / 86_400.0).round() as u64
            } else {
                0
            }
        } else {
            0
        };

        text.push_str(&format!(
            "{}. <code>{}</code>\n   Score: {:.2} | Depth: {} | Dir: {}{}{}\n",
            i + 1,
            addr,
            score,
            d,
            dir,
            if last_seen > 0 {
                format!(" | Last seen: {}d", last_seen_days)
            } else {
                "".to_string()
            },
            if shared_funders > 0 || shared_counterparties > 0 {
                format!(" | Funders: {} | Shared CP: {}", shared_funders, shared_counterparties)
            } else {
                "".to_string()
            }
        ));

        if let Some(reasons) = item.get("reasons").and_then(|v| v.as_array()) {
            if let Some(r0) = reasons.get(0).and_then(|v| v.as_str()) {
                text.push_str(&format!("   Evidence: {}\n", r0));
            }
        }

        text.push('\n');
    }

    text
}

fn format_wallet_cluster(wallet: &str, data: &serde_json::Value) -> String {
    let ingested = data
        .get("bootstrap_ingested_transactions")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let depth = data
        .get("analysis_depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let threshold = data
        .get("confidence_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let size = data.get("cluster_size").and_then(|v| v.as_u64()).unwrap_or(1);
    let strength = data
        .get("connection_strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let lookback_days = data
        .get("lookback_days")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    let mut text = format!(
        "🕸️ <b>Wallet Cluster</b>\n━━━━━━━━━━━━━━━━\n\n\
<b>Primary:</b>\n<code>{}</code>\n\n\
Cluster size: {} | Avg strength: {:.2}\nDepth: {} | Threshold: {:.2} | Lookback: {}d\nBootstrap ingested: {} tx\n\n",
        wallet, size, strength, depth, threshold, lookback_days, ingested
    );

    let Some(arr) = data.get("wallets").and_then(|v| v.as_array()) else {
        text.push_str("No results yet.");
        return text;
    };

    if arr.len() <= 1 {
        text.push_str("No cluster expansion yet. Try again in a minute (or increase bootstrap_limit on the API call).\n");
        return text;
    }

    for (i, item) in arr
        .iter()
        .filter(|w| {
        w.get("address").and_then(|v| v.as_str()) != Some(wallet)
    })
        .take(10)
        .enumerate()
    {
        let addr = item
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let d = item.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
        let dir = item
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let shared_funders = item
            .get("shared_funders")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let shared_counterparties = item
            .get("shared_counterparties")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let last_seen = item
            .get("last_seen_epoch")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let last_seen_days = if last_seen > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now > last_seen {
                ((now - last_seen) as f64 / 86_400.0).round() as u64
            } else {
                0
            }
        } else {
            0
        };

        text.push_str(&format!(
            "{}. <code>{}</code>\n   Score: {:.2} | Depth: {} | Dir: {}{}{}\n",
            i + 1,
            addr,
            score,
            d,
            dir,
            if last_seen > 0 {
                format!(" | Last seen: {}d", last_seen_days)
            } else {
                "".to_string()
            },
            if shared_funders > 0 || shared_counterparties > 0 {
                format!(" | Funders: {} | Shared CP: {}", shared_funders, shared_counterparties)
            } else {
                "".to_string()
            }
        ));

        if let Some(reasons) = item.get("reasons").and_then(|v| v.as_array()) {
            if let Some(r0) = reasons.get(0).and_then(|v| v.as_str()) {
                text.push_str(&format!("   Evidence: {}\n", r0));
            }
        }

        text.push('\n');
    }

    text
}
