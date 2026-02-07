ALTER TABLE wallets
ADD CONSTRAINT bal_non_neg CHECK (balance >= 0)
