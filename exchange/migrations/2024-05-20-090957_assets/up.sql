CREATE TABLE IF NOT EXISTS equities (
    id INTEGER NOT NULL PRIMARY KEY,
    ticker TEXT NOT NULL,
    description TEXT
);

CREATE TABLE IF NOT EXISTS equity_options (
    underlying INTEGER NOT NULL,
    expiration_date DATE NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price INT NOT NULL,
    exercise_style TEXT NOT NULL,
    PRIMARY KEY(underlying, expiration_date, contract_type, strike_price, exercise_style),
    FOREIGN KEY(underlying) REFERENCES equities(id)
);
