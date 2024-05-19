CREATE TABLE IF NOT EXISTS equities (
    ticker TEXT NOT NULL,
    description TEXT,
    PRIMARY KEY(ticker)
);

CREATE TABLE IF NOT EXISTS equity_options (
    underlying TEXT NOT NULL,
    expiration_date TEXT NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price REAL NOT NULL,
    exercise_style TEXT NOT NULL,
    FOREIGN KEY(underlying) REFERENCES equities(ticker)
);
