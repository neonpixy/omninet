# Lattice/

Placeholder for CrystalKit's bridge to the **Regalia** layout framework. No source files exist yet.

The layout system itself lives in the standalone Regalia package (`/Developer/Regalia/`). The standalone Ascension package (`/Developer/Ascension/`) is the existing bridge between Regalia and CrystalKit (FacetSanctum, FacetClansman, AscensionView).

## Architecture

```
Regalia (standalone)        CrystalKit (standalone)
Layout framework            Glass rendering
        \                   /
         \                 /
      Ascension (bridge package at /Developer/Ascension/)
      imports both
      FacetSanctum, FacetClansman, AscensionView
```

CrystalKit does NOT depend on Regalia. Ascension depends on both.

## See Also

- `/Developer/Regalia/CLAUDE.md` -- Full Regalia vocabulary and architecture
- `/Developer/Ascension/CLAUDE.md` -- Bridge package docs
