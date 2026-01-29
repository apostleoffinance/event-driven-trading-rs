use rust_decimal::Decimal;
use crate::error::{Result, TradingError};
use crate::portfolio::portfolio::Portfolio;
use crate::portfolio::position::PositionSide;
use super::PortfolioLimits;

#[derive(Debug)]
pub struct RiskEngine {
    account_balance: Decimal,
    portfolio: Portfolio,
    limits: PortfolioLimits,
    kill_switch: bool,
    kill_switch_reason: Option<String>,
    daily_loss: Decimal,
    peak_equity: Decimal,
}

impl RiskEngine {
    pub fn new(account_balance: Decimal, limits: PortfolioLimits) -> Result<Self> {
        if account_balance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Account balance must be positive".to_string(),
            ));
        }

        Ok(Self {
            account_balance,
            portfolio: Portfolio::new(),
            limits,
            kill_switch: false,
            kill_switch_reason: None,
            daily_loss: Decimal::ZERO,
            peak_equity: account_balance,
        })
    }

    pub fn account_balance(&self) -> Decimal {
        self.account_balance
    }

    pub fn equity(&self) -> Decimal {
        self.account_balance + self.portfolio.unrealized_pnl()
    }

    pub fn open_positions(&self) -> usize {
        self.portfolio.open_positions()
    }

    pub fn is_kill_switch_active(&self) -> bool {
        self.kill_switch
    }

    pub fn kill_switch_reason(&self) -> Option<&str> {
        self.kill_switch_reason.as_deref()
    }

    pub fn activate_kill_switch(&mut self, reason: impl Into<String>) {
        self.kill_switch = true;
        self.kill_switch_reason = Some(reason.into());
    }

    pub fn deactivate_kill_switch(&mut self) {
        self.kill_switch = false;
        self.kill_switch_reason = None;
    }

    pub fn update_price(&mut self, symbol: &str, price: Decimal) -> Result<()> {
        self.portfolio.update_price(symbol, price)?;
        self.update_risk_state()?;
        Ok(())
    }

    pub fn pre_trade_validate(
        &mut self,
        symbol: &str,
        side: PositionSide,
        entry_price: Decimal,
        position_size: Decimal,
        stop_loss_distance: Decimal,
    ) -> Result<()> {
        let _ = symbol;
        if self.kill_switch {
            return Err(TradingError::Risk(
                "Kill-switch active; trading halted".to_string(),
            ));
        }

        if entry_price <= Decimal::ZERO || position_size <= Decimal::ZERO || stop_loss_distance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Entry price, position size, and stop loss distance must be positive".to_string(),
            ));
        }

        if !self.limits.can_open_new_position(self.portfolio.open_positions())? {
            let msg = "Max open positions reached".to_string();
            self.activate_kill_switch(msg.clone());
            return Err(TradingError::Risk(msg));
        }

        let notional = entry_price * position_size;
        if self.limits.is_position_too_large(notional)? {
            return Err(TradingError::Risk(
                "Position notional exceeds limit".to_string(),
            ));
        }

        let projected_exposure = self.portfolio.exposure() + notional;
        let equity = self.equity();
        if equity <= Decimal::ZERO {
            self.activate_kill_switch("Equity depleted".to_string());
            return Err(TradingError::Risk("Equity depleted".to_string()));
        }

        let used_leverage = (projected_exposure / equity).round_dp(8);
        if self.limits.is_leverage_exceeded(used_leverage)? {
            return Err(TradingError::Risk(
                "Leverage exceeds limit".to_string(),
            ));
        }

        self.update_risk_state()?;
        if self.limits.is_daily_loss_exceeded(self.daily_loss)? {
            let msg = "Daily loss limit exceeded".to_string();
            self.activate_kill_switch(msg.clone());
            return Err(TradingError::Risk(msg));
        }

        let _ = side;
        Ok(())
    }

    pub fn record_trade_open(
        &mut self,
        symbol: String,
        side: PositionSide,
        entry_price: Decimal,
        position_size: Decimal,
        stop_loss: Decimal,
        opened_at: u64,
    ) -> Result<()> {
        self.portfolio.open_position(
            symbol,
            side,
            entry_price,
            position_size,
            stop_loss,
            opened_at,
        )
    }

    pub fn record_trade_close(&mut self, symbol: &str, exit_price: Decimal) -> Result<Decimal> {
        let pnl = self.portfolio.close_position(symbol, exit_price)?;
        self.account_balance += pnl;
        self.update_risk_state()?;
        Ok(pnl)
    }

    pub fn liquidate_all(&mut self) -> Vec<(String, Decimal, Decimal)> {
        let results = self.portfolio.close_all_at_last();
        for (_symbol, _exit_price, pnl) in &results {
            self.account_balance += *pnl;
        }
        let _ = self.update_risk_state();
        results
    }

    fn update_risk_state(&mut self) -> Result<()> {
        let equity = self.equity();
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        let loss = (self.account_balance - equity).abs();
        self.daily_loss = if equity < self.account_balance { loss } else { Decimal::ZERO };
        Ok(())
    }
}