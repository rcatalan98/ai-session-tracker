use crate::metrics::filter_by_period;
use crate::parser::Session;
use colored::Colorize;

/// Claude Opus 4.5 pricing per million tokens (as of 2026)
const INPUT_PRICE_PER_MILLION: f64 = 15.0;
const OUTPUT_PRICE_PER_MILLION: f64 = 75.0;

/// Calculate cost from token counts
pub fn calculate_cost(input_tokens: u64, output_tokens: u64) -> f64 {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * INPUT_PRICE_PER_MILLION;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * OUTPUT_PRICE_PER_MILLION;
    input_cost + output_cost
}

/// Format cost as USD
fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// Format token count with commas
fn format_tokens(count: u64) -> String {
    let s = count.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Print cost summary for sessions
pub fn print_cost_summary(sessions: &[Session], period: &str, detailed: bool) {
    let filtered = filter_by_period(sessions, period);

    if filtered.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    // Calculate totals
    let total_input: u64 = filtered.iter().map(|s| s.token_input).sum();
    let total_output: u64 = filtered.iter().map(|s| s.token_output).sum();
    let total_cost = calculate_cost(total_input, total_output);

    // Header
    println!("{}", "TOKEN USAGE & COST".bold());
    println!("{}", "═".repeat(50));
    println!(
        "Period: {} | Sessions: {}",
        period.bold(),
        filtered.len().to_string().bold()
    );
    println!();

    // Summary
    println!("{}", "SUMMARY".bold());
    println!("{}", "─".repeat(30));
    println!(
        "Input tokens:   {:>15} ({})",
        format_tokens(total_input),
        format_cost((total_input as f64 / 1_000_000.0) * INPUT_PRICE_PER_MILLION).dimmed()
    );
    println!(
        "Output tokens:  {:>15} ({})",
        format_tokens(total_output),
        format_cost((total_output as f64 / 1_000_000.0) * OUTPUT_PRICE_PER_MILLION).dimmed()
    );
    println!("{}", "─".repeat(30));
    println!(
        "Total cost:     {:>15}",
        format_cost(total_cost).green().bold()
    );
    println!();

    // Pricing note
    println!(
        "{}",
        format!(
            "Pricing: ${}/M input, ${}/M output (Claude Opus 4.5)",
            INPUT_PRICE_PER_MILLION as u32, OUTPUT_PRICE_PER_MILLION as u32
        )
        .dimmed()
    );
    println!();

    // Detailed breakdown if requested
    if detailed {
        println!("{}", "PER-SESSION BREAKDOWN".bold());
        println!("{}", "─".repeat(70));
        println!(
            "{:<12} {:>15} {:>15} {:>12}",
            "SESSION".dimmed(),
            "INPUT".dimmed(),
            "OUTPUT".dimmed(),
            "COST".dimmed()
        );

        // Sort sessions by cost (descending)
        let mut sorted: Vec<_> = filtered.iter().collect();
        sorted.sort_by(|a, b| {
            let cost_a = calculate_cost(a.token_input, a.token_output);
            let cost_b = calculate_cost(b.token_input, b.token_output);
            cost_b
                .partial_cmp(&cost_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for session in sorted.iter().take(20) {
            let session_cost = calculate_cost(session.token_input, session.token_output);
            let session_short: String = session.session_id.chars().take(10).collect();

            println!(
                "{:<12} {:>15} {:>15} {:>12}",
                session_short,
                format_tokens(session.token_input),
                format_tokens(session.token_output),
                format_cost(session_cost)
            );
        }

        if sorted.len() > 20 {
            println!(
                "{}",
                format!("... and {} more sessions", sorted.len() - 20).dimmed()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost() {
        // 1M input tokens = $15
        assert_eq!(calculate_cost(1_000_000, 0), 15.0);
        // 1M output tokens = $75
        assert_eq!(calculate_cost(0, 1_000_000), 75.0);
        // Combined
        assert_eq!(calculate_cost(1_000_000, 1_000_000), 90.0);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1000), "1,000");
        assert_eq!(format_tokens(1234567), "1,234,567");
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.0001), "$0.0001");
        assert_eq!(format_cost(0.009), "$0.0090");
        assert_eq!(format_cost(0.05), "$0.05");
        assert_eq!(format_cost(1.50), "$1.50");
        assert_eq!(format_cost(15.0), "$15.00");
    }
}
