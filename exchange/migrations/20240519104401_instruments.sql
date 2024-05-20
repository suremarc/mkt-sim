CREATE TABLE IF NOT EXISTS equities (
    ticker TEXT NOT NULL PRIMARY KEY,
    description TEXT
);

CREATE TABLE IF NOT EXISTS equity_options (
    underlying TEXT NOT NULL,
    expiration_date TEXT NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price TEXT NOT NULL,
    exercise_style TEXT NOT NULL,
    FOREIGN KEY(underlying) REFERENCES equities(ticker),
    PRIMARY KEY(underlying, expiration_date, contract_type, strike_price, exercise_style)
);
