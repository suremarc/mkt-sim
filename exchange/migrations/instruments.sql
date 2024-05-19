CREATE TABLE IF NOT EXISTS equities (
    ticker TEXT NOT NULL,
    description TEXT
);

CREATE TABLE IF NOT EXISTS equity_options (
    FOREIGN (underlying) REFERENCES equities(ticker),
    expiration_date TEXT NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price REAL NOT NULL,
    exercise_style TEXT NOT NULL,
);
