export const widgetDescriptions = {
  strategy:
    "Growth and returns for the selected period, separated from cash additions.",
  exposure:
    "How much is invested, what is at risk, and which inputs are missing.",
  reconciliation:
    "How position P/L, income, expenses, and record adjustments add up to investment gain.",
};

export const widgetHelp = {
  portfolioEquity:
    "Cash plus the signed market value of your positions in this currency, cycle and account selection, through the To date. Long holdings add value; short liabilities subtract it. Reserved cash is still included. This is a snapshot, not a return for the From → To period.",
  totalPositionPL:
    "Realized P/L plus unrealized P/L for retained position history, including trade fees. Cumulative through To; From does not reset this value. Cash additions, withdrawals, dividends, interest and standalone expenses are not position P/L.",
  realizedPL:
    "Profit or loss on quantities already closed or settled, using the position ID's weighted-average basis and transaction fees. Cumulative through To; From does not reset this value. Imported records may cover only retained known-basis history.",
  unrealizedPL:
    "Profit or loss on quantities still open: their latest available value on or before To compared with remaining cost basis. This is a snapshot, not the change since From. Missing quotes leave the value unavailable; stale quotes are flagged.",
  strategy:
    "Uses From → To, or the first available dated valuation when From is empty. Growth removes net capital additions. Return method labels identify estimates; missing valuation or cash-flow evidence can leave results unavailable.",
  investmentGain:
    "The change in equity over the measured interval minus net capital added to this scope. Deposits are not investment profit. Trading P/L, income, expenses and record reconciliation contribute to this figure. The actual measured dates appear in the growth chart and reconciliation widget.",
  timeWeightedReturn:
    "Return over the measured period after adjusting for capital additions and withdrawals. It links interval returns to show investment performance separately from funding. This percentage is not annualized. The method note says whether the result is exact or an end-of-day-flow estimate.",
  moneyWeightedReturn:
    "Annualized XIRR: a return that reflects both the amounts and dates of capital movements, plus opening and ending equity. It describes the experience of the money invested. It can be unavailable when the inputs do not support a usable rate.",
  portfolioVolatility:
    "Annualized variation in portfolio returns after adjusting for capital movements, using the configured recent observation window and 252 trading days per year. It is not maximum possible loss. Too little dated return evidence leaves it unavailable; missing days are not silently bridged.",
  exposure:
    "Position exposure, cash and capital totals are snapshots through To; they do not reset at From. Income and expenses use the selected interval. Missing quotes, stops or option delta inputs can leave affected values unavailable. Exposure measures are not guarantees of maximum loss.",
  grossNetExposure:
    "Gross adds the absolute market values of all positions. Net adds signed values: longs are positive and shorts negative. Cash is excluded. These measure invested exposure, not the amount you could lose; options' directional exposure is shown separately under Delta exposure.",
  plannedLoss:
    "Estimated loss from the current marks to your valid stop prices, across open positions. An open position without a usable stop or quote makes the total unavailable. Stops are planning inputs, not guaranteed execution prices, so actual losses can be larger.",
  deltaExposure:
    "Signed directional exposure: stock market values plus options adjusted by delta, underlying price, contract count and multiplier. It approximates sensitivity to underlying price moves and changes as prices and deltas change. Missing option delta or underlying-price evidence makes the total unavailable.",
  externalContributions:
    "Initial funding plus external deposits minus withdrawals through To, for this selection and currency. Internal cycle/account transfers and currency conversions are excluded. This is contributed money, not investment gain; From does not reset it.",
  allocatedCapital:
    "Net capital supplied to this cycle/account/currency scope through To. Includes external funding and net transfers or conversions into and out of the scope. Internal movements cancel when both endpoints are in the same scope. From does not reset this value.",
  cashBalance:
    "Recorded cash through To after trade cash effects and cash movements, including reserved cash. It is not the amount available to trade; the Cash page separates reserved and available balances. From does not reset this snapshot.",
  incomeExpenses:
    "Dividends and interest received / standalone expenses recorded in the selected interval through To. These are separate from position P/L. Transaction fees already included in trade accounting are not counted again here.",
  riskCoverage:
    "Counts open positions missing usable stops and positions missing delta evidence. Missing evidence means the related risk total may be unavailable, not zero. An uncovered short stock or call can have unbounded loss even when some exposure measures are known.",
  reconciliation:
    "Reconciles equity-based investment gain over the displayed interval with the position ledger: position P/L + income − expenses + reconciliation = total investment gain. Each row uses the same measured interval and native currency.",
  periodPositionPL:
    "Realized P/L earned during the displayed interval plus the change in unrealized P/L, adjusted for basis corrections. This interval value differs from the cumulative Total position P/L card. Missing dated marks at either endpoint can leave it unavailable.",
  incomeMinusExpenses:
    "Dividends and interest minus standalone expenses between the measured start and To. Trade fees are already reflected in position P/L and are not subtracted a second time.",
  reconciliationAdjustment:
    "The remaining difference between equity-based investment gain and position P/L plus income minus expenses. It can reflect imported baselines, basis corrections or differences in cash records. It is not automatically trading profit or FX profit; a nonzero amount is a reason to inspect the records.",
  totalInvestmentGain:
    "Position P/L over the displayed interval + income − expenses + baseline/basis/cash reconciliation. This is the equity-based gain after removing net capital additions, and matches Investment gain in Strategy performance for the same native-currency view.",
};
