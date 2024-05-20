CREATE TABLE IF NOT EXISTS equities (
    ticker TEXT NOT NULL,
    description TEXT,
    PRIMARY KEY(ticker)
);

CREATE TABLE IF NOT EXISTS equity_options (
    underlying TEXT NOT NULL,
    expiration_date TEXT NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price INT NOT NULL,
    exercise_style TEXT NOT NULL,
    PRIMARY KEY(underlying, expiration_date, contract_type, strike_price, exercise_style),
    FOREIGN KEY(underlying) REFERENCES equities(ticker)
);
