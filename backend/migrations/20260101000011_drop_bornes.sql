-- Retrait des bornes matin/midi/aprem/soir (§4.2 point 4) : ne servaient
-- qu'aux règles "non-enchaînement midi/soir", devenues caduques avec
-- l'introduction des créneaux repas (§4.4), et n'étaient utilisées par
-- rien d'autre.
ALTER TABLE evenement DROP COLUMN born_fin_matin;
ALTER TABLE evenement DROP COLUMN born_fin_midi;
ALTER TABLE evenement DROP COLUMN born_fin_aprem;
