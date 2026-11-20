# Bristle solver prototypes

Throwaway CPU prototypes (Python + numpy) used in phase 1 of #62 to choose the bristle
solver. They model one strand of 8 vertices at the engine's scale (40 µm/px) with real
hair properties (60–75 µm diameter, E ≈ 4 GPa). Run each with `python3 <file>`.

| File | What it shows |
|---|---|
| `strand.py` | XPBD strand (stretch, DER-style bend, clamp), one Gauss-Seidel sweep per substep. Tip load diverges on the first substep. |
| `free.py`, `dbg.py` | XPBD free strand: stable root→tip, diverges when the sweep direction alternates; a spurious ~100 ms mode from solver convergence. |
| `disp.py`, `scale.py` | XPBD under displacement and force control with the mass scaled 10²–10⁶×: statics stay wrong unless h²·w·\|∇C\|² ≪ α. |
| `direct.py` | Direct solve in λ form (Deul et al. 2018 style): fixes stiffness, but contact outside the solve fights it, and the compliance falls below f32 precision. |
| `qs.py` | Quasi-static strand in hinge coordinates (Baxter & Lin, Chu & Tai style): cantilever FL³/3EI within 1%. |
| `contact.py` | Same, with frictionless contact through an active set: tilted contact force within 2–3.5%, upright buckling stable and inextensible. |
| `head.py` | Whole-head stiffness vs hair diameter without friction (0.2–0.9 N at ~90°). |

The conclusions and the GPU numbers are in the #62 comment that links here.
