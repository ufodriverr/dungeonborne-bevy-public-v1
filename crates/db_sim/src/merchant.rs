//! The merchant's goods: what `ExcelMarketInfo`'s merchant (market 101, "商人") sells for gold, from
//! `ExcelMarketProductItem` (CONFIRMED tables). Its tabs hold fixed products (always offered) and pooled ones
//! (`refresh_pool_id`), of which the tab shows `random_count` per refresh (`ExcelMarketTabInfo`, `period` one day).
//! HYPOTHESIS: the port refreshes the pooled picks per run (by a seed the profile gives) and draws them by their
//! `random_weight`; the dated weekly specials (`buy_start_timepoint` / `buy_end_timepoint`, tabs 1009-1011, all of
//! 2024), products with any cost but gold (`cost_type` 1001) or none (100060 at 0), and stock limits
//! (`refresh_count`) are left out.

use db_formats::ConfigTables;

/// `ExcelMarketInfo`'s merchant.
pub const MERCHANT: i32 = 101;
/// The gold cost type (the merchant's prices are all in it but one tab's second option).
const GOLD: i32 = 1001;

/// One thing the merchant offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub product_id: i32,
    pub tab: i32,
    pub item_id: i32,
    pub count: u16,
    pub price: u64,
}

/// The merchant's offers for refresh `seed`, in tab and display order.
pub fn offers(t: &ConfigTables, seed: u64) -> Vec<Offer> {
    let Some(market) = t.excel_market_info.iter().find(|m| m.market_id == MERCHANT).and_then(|m| m.data.as_ref()) else { return Vec::new() };
    let mut out = Vec::new();
    for &tab in &market.market_tab_ids {
        let picks = t.excel_market_tab_info.iter().find(|x| x.market_tab_id == tab).and_then(|x| x.data.as_ref()).map_or(0, |x| x.random_count.max(0) as usize);
        let mut fixed = Vec::new();
        let mut pooled = Vec::new();
        for row in t.excel_market_product_item.iter().filter(|r| r.market_tab_id == tab) {
            let Some(p) = row.product.as_ref().filter(|p| p.is_on_sale && p.buy_start_timepoint.is_empty()) else { continue };
            let Some(price) = p.cost_options.first().filter(|o| o.cost_items.len() == 1 && o.cost_items[0].cost_type == GOLD).map(|o| o.cost_items[0].cost_price.max(0) as u64).filter(|&p| p > 0) else { continue };
            let Some(reward) = p.reward_items.first().filter(|_| p.reward_items.len() == 1) else { continue };
            let offer = Offer { product_id: row.product_id, tab, item_id: reward.type_id, count: reward.count.clamp(1, i32::from(u16::MAX)) as u16, price };
            if row.refresh_pool_id == 0 { fixed.push((p.display_order, offer)) } else { pooled.push((p.random_weight.max(1) as u64, offer)) }
        }
        fixed.sort_by_key(|f| f.0);
        out.extend(fixed.into_iter().map(|f| f.1));
        // The pooled picks: weighted, without repeats, from a seed per refresh and tab.
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(tab as u64 | 1);
        for _ in 0..picks.min(pooled.len()) {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let total: u64 = pooled.iter().map(|p| p.0).sum();
            let mut roll = (state >> 33) % total.max(1);
            let i = pooled.iter().position(|p| roll < p.0 || { roll -= p.0; false }).unwrap_or(0);
            out.push(pooled.remove(i).1);
        }
    }
    out
}
