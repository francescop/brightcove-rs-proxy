CREATE TABLE videos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    name VARCHAR(50) not null,
    thumbnail VARCHAR(50),
    numero_corsa VARCHAR(5),
    data TEXT not null,
    categorie TEXT,
    tipologia TEXT,
    cavalli TEXT,
    fantini TEXT,
    primo VARCHAR(50),
    secondo VARCHAR(50),
    terzo VARCHAR(50),
    ippodromo VARCHAR(50),
    video_views INTEGER default 0,
    distanza VARCHAR(10),
    terreno VARCHAR(20),
    bc_video_id TEXT UNIQUE not null
)
