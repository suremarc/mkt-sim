CREATE SEQUENCE asset_id;

CREATE TABLE IF NOT EXISTS equities (
    id INTEGER PRIMARY KEY DEFAULT nextval('asset_id'),
    ticker TEXT NOT NULL UNIQUE,
    description TEXT,
    created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS equity_options (
    id INTEGER PRIMARY KEY DEFAULT nextval('asset_id'),
    underlying INTEGER NOT NULL,
    expiration_date DATE NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price INT NOT NULL,
    created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(underlying, expiration_date, contract_type, strike_price),
    FOREIGN KEY(underlying) REFERENCES equities(id)
);
