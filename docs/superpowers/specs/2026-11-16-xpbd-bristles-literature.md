# Bristle physics for a real-time GPU oil brush: literature review

Scope: primary sources for (1) touch and pressure, (2) bristle motion, (3) the mark left, for a wgpu/WGSL compute engine that simulates a few hundred hairs as short vertex chains (cadmium currently uses `BRISTLE_VERTICES = 6`).

Verification labels:
- **[V]** I read the primary PDF myself (text extracted with pdftotext). Equations and numbers are quoted from it.
- **[P]** Taken from a primary but indirect source (patent text, the authors' abstract or project page) or from another paper's description of the work.
- **[U]** Not verified from a primary source: my own recollection, a secondary source, or my own derivation. Treat as a hypothesis.

---

## Part A: Virtual paint-brush models

### A1. Baxter, Scheib, Lin, Manocha: "DAB: Interactive Haptic Painting with 3D Virtual Brushes", SIGGRAPH 2001 [V]
- PDF: https://discmaster.textfiles.com/file/16660/SIGGRAPH%202001%20DVD.iso/Papers/baxter.pdf (DOI 10.1145/383259.383313)
- **Model:** a spring-mass particle skeleton (a single spine, or a pyramidal truss for rounds) skinned with a subdivision surface.
- **Integrator:** the Desbrun-Schröder-Barr approximate implicit integrator. Step 1 integrates the linear forces implicitly, step 2 post-corrects for the non-linear ones, step 3 applies deformation constraints that iteratively shrink over-stretched springs. The authors note that it "effectively ignores stiff bend forces" for some skeletons, which gives too much angular motion.
- **"Aristotelian dynamics" (key insight):** "Real bristles empirically obey the Aristotelian model of physics… lack of inertia… objects move only for the duration that forces are applied." They integrate the first-order system ẋ = M⁻¹f in place of second-order dynamics, so the brush is heavily damped by construction.
- **Contact:** penetrating particles are projected to the canvas surface. Friction is a drag force, F_friction = −μ‖F_normal‖ v_tangential.
- **Pressure / first contact (haptic model):** force is piecewise in penetration depth d_p. A light spring k₁/l_p acts while d_p < l_p, and a stiff k₂ "handle hits canvas" term acts beyond that. The model is then extended to a 4-segment Hermite curve. "When a real brush contacts the canvas at close to a right angle, the stiff bristles initially act as strong compressive springs… As more pressure is applied, the bristles buckle and the compressive force reduces as bending forces take over." The initial compressive stiffness is scaled by an angle-of-contact factor.
- **Cost:** deformation at about 30 Hz and haptics at 1 kHz, decoupled.
- **What transfers:** the first-order, overdamped character of bristles, and a phenomenological account of first contact (stiff axial phase, then buckling, then bending). A positional solver reproduces the buckling only if the hairs are not perfectly straight and aligned with the canvas normal (see synthesis, item 5).

### A2. Baxter and Lin: "A Versatile Interactive 3D Brush Model", Pacific Graphics 2004 [V]
- PDF: https://gamma.cs.unc.edu/BRUSH/Baxter-Lin-PG04-Submission.pdf. Project page: https://gamma.cs.unc.edu/BRUSH
- **Model:** fewer than 10 simulated spines, each a 3-joint kinematic chain with 2 Euler-XYZ angles per joint (θ, φ). XYZ was chosen because its singularity sits at 90°, not at rest. Twist is ignored. Up to hundreds of render-only bristle strips are interpolated from the spines: p_interp(s) = Σ wᵢ pᵢ(s), with weights from a least-squares fit min Σ‖p_interp(0) − pᵢ(0)‖² wᵢ², Σwᵢ = 1, wᵢ ≥ 0.
- **Dynamics:** quasi-static. "Given an externally applied force system, a brush will reach equilibrium very rapidly. Thus we can approximate the dynamics by solving a static equilibrium problem every time step." Each step: (1) move the handle, (2) move the spines rigidly with it, (3) minimize energy. The procedure is a "backward search for the closest configuration… in which spring forces were equal and opposite to friction forces."
  - E = E_s + E_f + E_d
  - E_s = Σ Kᵢ β(θᵢ, φᵢ)²/2, with β = acos(cosθ cosφ)
  - E_f = Σ μ |F_n,i| ‖Δx_c,i‖ (Coulomb work along a straight-line slip of the contact point)
  - E_d = Σ Dᵢ |Δβᵢ| (internal friction)
  - constraint (xᵢ − x_p)·n̂_p ≥ 0
  - Solved by SQP with an active set and a BFGS Hessian.
- **Stiction:** solve with μ_s. If the contact moves (Δx_c > 0), switch to sliding and re-solve with μ_k. Return to sticking when Δx_c < ε. Hysteresis is suggested.
- **Anisotropic friction (tip catches when pushed):** E_f = (1 − η) μ|F_n|‖Δx_c‖, with η = C_η · max(0, d_p · Δx̂_c)^k and C_η ∈ [0,1]. Here d_p is the preferred "pull" direction. The form is Blinn-Phong-like and C¹.
- **Plasticity:** internal joint friction F_d = −Dᵢ sgn(Δβᵢ). Because friction exceeds the spring force at small angles, the brush "does not return all the way" to rest.
- **Variable stiffness:** Kᵢ is higher near the ferrule. Hairs are thinner at the tip, packing in the ferrule stiffens the base, and not every hair reaches the tip.
- **Physical scaling (useful numbers):** "a brush under 1-2 Newtons of normal force should bend nearly 90°" gives K_s ≈ **5–10 N·mm/rad**. With these, "reasonable surface friction coefficients μ are about **0.3–0.5**, and internal friction constants Dᵢ a maximum of about **0.01**."
- **Mark:** strips are rendered orthographically from the canvas with a clip plane near the surface. Strips are sorted back to front along the mean tip-deflection direction, a cheap stand-in for hair-hair collision ordering.
- **Cost:** a 1 GHz Pentium 4, 5 SQP iterations, about 100 µs per 3-joint spine, about 10 spines plus 200 interpolated strips.
- **What transfers:** μ, stiffness and plasticity magnitudes; anisotropic and static/kinetic friction formulas; the quasi-static view; and the warning that interpolated hairs cannot produce individual-bristle streaks unless they are simulated.

### A3. Baxter: PhD thesis, "Physically-Based Modeling Techniques for Interactive Digital Painting", UNC 2004 [V, partially read]
- PDF: https://www.billbaxter.com/dissertation/Baxter-dissertation.pdf
- Collects DAB, the PG04 brush, Stokes Paint and IMPaSTo.
- **Paint:** a three-regime model. Paint is solid-like at rest, a high-viscosity Stokes fluid (∂u/∂t = ν∇²u − ∇p, ∇·u = 0), and shear-thinned near the brush, implemented as a filter.
- **Viscosity:** "The viscosity of typical oil paints directly out of the tube is over 10,000 cSt." Baxter flags this himself as an analogy to toothpaste and molasses: "It is difficult to obtain much information about the fluid dynamic properties of artistic paints." With V = 0.3 m/s and L = 0.01 m, Re ≈ 0.3, so Stokes flow is justified. **[V for the quote; the value is an estimate, U]**

### A4. Baxter and Govindaraju: "Simple Data-Driven Modeling of Brushes", I3D 2010 [V]
- PDF: https://www.microsoft.com/en-us/research/wp-content/uploads/2010/02/Baxter-DataDrivenBrushes-I3D10.pdf
- **Model:** control bristles deform in a plane. Each deformation is one cubic Bézier, measured from video of a real brush pressed on glass and tabulated by (root angle θ, normalized handle height ĥ). ĥ = 1 at first touch and 0 when the root reaches the canvas. The table is looked up with bilinear interpolation.
- **Closed-form friction plus bend blend:** take p_f (last-step tip position, the friction optimum) and p_b (the frictionless table optimum). Minimizing E = μ f_N‖p_f − x‖ + k_b‖p_b − x‖² gives x = p_f + ξ(p_b − p_f), with **ξ = max(0, (k_b‖p_b − p_f‖ − μ f_N) / (k_b‖p_b − p_f‖))**.
- **Values used:** k_b = 1, μ = 0.75, f_N = 1. A compression-based variant uses f_N = 0.7 + 20κ², μ = 0.2, k_b = κ, where κ is the compression ratio.
- **Cost:** under 10 µs per spine, 5–45 times faster than optimization.
- **What transfers:** the closed-form ξ is a per-vertex stick/slip rule that fits a shader, and it is in effect the static-friction projection that XPBD papers also use. The height normalization ĥ matches the pressure-mapping idea in A7.

### A5. Chu and Tai: "An Efficient Brush Model for Physically-Based 3D Painting", Pacific Graphics 2002 [V]
- PDF: https://cse.hkust.edu.hk/VCB/PG02.pdf
- **Model:** a spine of segments that get shorter toward the tip (more resolution where it bends). Each joint has a bend angle θ and a turn angle φ. Each spine node has 2 lateral nodes that slide on a line in the joint-bisecting plane and model flattening and spreading. The surface is swept from two half-ellipses. Area conservation sets the minor radius c = 2r²/(a + b + r_left + r_right).
- **Dynamics:** constrained energy minimization by SQP: E = E_deform + E_frict. Bristles "have very little inertia relative to the forces applied onto them… a highly damped system", so they avoid stiff ODEs.
  - **Bend spring energy:** κ_bend|θ|^m with **m = 3** "to account for the non-linearity of real bristles". This makes the tuft soft for small bends and stiff for large ones.
  - **Lateral stretch spring (wetness):** κ_stretch |d − (r + s(θ))|^m. This term models "the attraction force of water molecules that pulls the lateral nodes towards the spine", and κ_stretch is a linear function of wetness. *This is the only explicit wetness-to-cohesion law among the brush papers.*
  - **Friction:** E_frict = μ Σ F·(κ_f Δx_par + (1 − κ_f) Δx_perp), anisotropic with larger sideways friction because the tuft surface is corrugated. The normal force F of the first contacting node is the sum of the vertical spring-force components above it. Part of F is shared with the upper neighbour, in proportion to how much of the segment touches the paper, to avoid discontinuities.
  - **Plasticity by zero-shifting:** BendEnergy(θ) = k|θ − ρ|³ with **ρ = min(θ′, α)**, where θ′ is the previous frame's angle. Strokes are shown for α = 0, 0.05, 0.08 and 0.10 rad. α can follow wetness.
  - **Pore resistance:** a moving blocking-plane constraint in front of the tip whose distance depends on tip height and angle. It makes pushed strokes rough and lets the tuft be straightened into a blade.
- **Ink:** values stored per node. The footprint is the clipped brush surface rendered orthographically. An alpha map on the surface fakes bristle splitting, and lowering it mimics dry brush.
- **Cost:** 25 fps on a 1 GHz Pentium III with a GeForce2. Typically fewer than 10 SQP iterations; 98% of frames need fewer than 20.
- **What transfers:** the |θ|³ non-linear bending, the zero-shift plasticity rule, wetness-dependent cohesion, anisotropic friction and the blocking-plane idea for pushed strokes. Their observation also matters: a wet brush forms one tip, and as it dries the tip "starts to split into two or more tips", which produces "flying white" (dry-brush gaps).

### A6. Chu and Tai: "Real-time Painting with an Expressive Virtual Chinese Brush", IEEE CG&A 24(5), 2004; and MoXi (SIGGRAPH 2005) [P]
- Project page: https://cse.hkust.edu.hk/VCB/ (DOI 10.1109/MCG.2004.37)
- From the abstract: a deformable 3D brush with spreading bristles, plasticity and paper pore resistance, in real time on consumer hardware. It appears to be the journal version of A5. **I did not read the full text.**
- **MoXi** models ink dispersion in absorbent paper with a lattice-Boltzmann fluid. It is not a brush model and has low relevance to oil. **[U, from memory]**

### A7. DiVerdi, Krishnaswamy, Hadap: "Industrial-Strength Painting with a Virtual Bristle Brush", VRST 2010, pp. 119–126 (DOI 10.1145/1889863.1889889) [P]
- The PDF is closed access (ACM). I could not obtain it, so this entry is reconstructed from:
  - the Adobe abstract: "efficient formulation of bristle behaviors as strand dynamics in a non-inertial reference frame", integrated into commercial Adobe painting and vector apps and evaluated by professionals (https://research.adobe.com/publication/industrial-strength-painting-with-a-virtual-bristle-brush);
  - Wetbrush's description: "simulated 3D brush bristles in a non-inertial brush frame, but handled paint and the actual painting process in 2D";
  - the same group's patents (below), filed 2009–2010, which describe the system.
- **US 2011/0181618 / US 8,830,242, "Realistic Real-Time Simulation of Brush Behavior" (DiVerdi, Hadap)** [P] (https://patents.google.com/patent/US20110181618A1/en):
  - Each bristle is a chain of about 4–5 thin rigid links joined by ball-and-socket joints, with the first joint attached to the handle. Few links are used because "real bristles rarely form tight curves".
  - Articulated rigid-body dynamics with hard constraints in ODE. Joint CFM/ERP are set to behave like a spring-damper toward rest angles, or explicit angular spring torques are used. Stiffness varies along the bristle (stiff base, soft tip). The handle is driven toward the target pose by computed forces.
  - Implicit integration for stability. Each bristle has its own handle body, so bristles are independent islands.
  - **40 bristles at 60 Hz, or 75 at 30 Hz, on one CPU core.** Brushes have 50–100 bristles. dWorldQuickStep (20 iterations) costs about 300 units against about 3375 for the direct solver.
  - The canvas is a plane. Inter-bristle contact is optional (contact joints or penalty forces). The mark is each link's swept quad, projected and clipped at a height threshold, with a 1-px minimum line.
- **US 8,760,438, "Simulating stiff bristle brushes using stiffness-height parameterization"** [P] (https://patents.google.com/patent/US8760438B2/en). This is directly relevant to light touch:
  - Normalized stylus pressure p ∈ [0,1] maps to brush height, not force. **p = 0 means the bristle tips are just above the canvas (h_min); p = 1 means pressed fully (h₀).**
  - Eq. 1: h(p) = −p + 1
  - Eq. 2: s′ = 2s − 1
  - Eq. 3: h(p, s′) = h(p) for s′ < 0; h(p, s′) = (1 − s′)h(p) + s′h_min for s′ ≥ 0
  - In words: brushes above 50% stiffness press in less for the same pressure, and at maximum stiffness they cannot be deformed. In mixed brushes, the stiffest bristles limit how far the brush goes in.
- **Hadap, "Oriented Strands: Dynamics of Stiff Multi-Body System", SCA 2006** (https://diglib.eg.org/handle/10.2312/SCA.SCA06.091-100): a recursive, linear-time, fully implicit articulated-strand solver. It is plausibly the "strand dynamics" in DiVerdi 2010, but **[U] not confirmed**.
- **What transfers:** simulate hairs in the ferrule frame, and treat pressure as a height or penetration target with "p = 0 equals tip just touching". The second rule removes the "light touch lands the belly" failure, because the tip is the first thing to touch.

### A8. Chen, Kim, Ito, Wang: "Wetbrush: GPU-based 3D Painting Simulation at the Bristle Level", ACM TOG 34(6) (SIGGRAPH Asia 2015) [V]
- PDF: https://wanghmin.github.io/publication/chen-2015-wgb/Chen-2015-WGB.pdf. Page: https://wanghmin.github.io/publication/chen-2015-wgb
- **Bristles:** "40 to 600 bristles, and each bristle contains **10 vertices** and **128 samples**."
- **Non-inertial brush frame (Eq. 2):**
  v̇_Bi = R_B a_i − β_B (v̇_B + ω_B×(ω_B×x_Bi) + ω̇_B×x_Bi + 2ω_B×v_Bi)
  The terms are the rectilinear, centrifugal, Euler and Coriolis accelerations. β_B = 1 is Newtonian; β_B = 0 means "forces the vertex to stay with the brush in a position-based way". **Typical β_B ∈ [0, 0.1]**, and β_B = 0 for small stiff brushes "to draw thin and clear lines". The step integrates explicitly in the brush frame, then enforces "inextensible and bending constraints… by the position-based method [Müller 2007]", then transforms back.
- **Clustering and thickness (no collision detection):** "estimate the bristle vertex density at each vertex using a smoothed kernel function, and then enforce a **minimal density constraint**… [Macklin and Müller 2013]". This means a position-based-fluids-style unilateral density constraint on bristle vertices.
- **Bristle samples:** each bristle is a cubic Hermite spline through its vertices (finite-difference tangents), sampled so that sample spacing is below one pixel. Frames come from minimal twist (Bishop, as in DER). The samples are rasterized into density and velocity fields that serve as boundary conditions for the paint.
- **Paint load per sample (dry brush and streaks):** load m_j, capacity M_j, pigment c_j.
  - Eq. 12: M⁰_j = max((1 − μψ(x_j)) M_max, M_min). ψ is the bristle-sample density, so compressed bristles hold less paint. μ = 0.5.
  - Eq. 13: M_j = M⁰_j − ρ₀π(R_j − r_j)²(R_j − (R_j − r_j)/3), with R_j = (3M⁰_j / 4πρ₀)^{1/3} and **ρ₀ = 1.0×10³ kg/m³**. This is a spherical cap cut off by the canvas or paint surface, so pressing a sample onto the canvas squeezes paint out.
- **Paint:** hybrid. Particles live within D₀ = 1 cm of bristles; beyond that, a 4096×4096×64 Eulerian density grid. Particle adhesion is handled in the bristle-sample frame within D₁ = 0.3 cm (β_L ∈ [0, 0.2]). Particle viscosity friction is f = −max((1 − δd_k)², 0) v_k with δ = 1/0.2 cm. Pressure projection uses a fixed-point accelerated Jacobi (3 fixed-point = 6 Jacobi iterations, α = 1). Dryness and oil density are per cell; oil density controls viscosity.
- **Cost:** 30–110 fps on a GTX TITAN X with up to 2M particles. The fluid dominates; the bristle cost is not reported separately. The authors list the speed cap on strokes as a main drawback. Shear thinning is not modelled.
- **What transfers:** almost everything except the 3D fluid. Use the brush-frame integration with β_B; PBD inextensibility and bending; the density constraint for hair thickness and clumping; sub-pixel Hermite sampling for streaks; and capacity that depends on compression and canvas proximity (eqs. 12–13) for dry-brush gaps and pressure-dependent deposit.

### A9. Other brush models (low priority)
- **Saito and Nakajima: "3D Physics-Based Brush Model for Painting", SIGGRAPH 99 Sketches, p. 226** (DOI 10.1145/311625.312110) [P, secondary]. A single Bézier spine with discs, solved by energy optimization (spring, friction and kinetic terms). It is the ancestor of A2 and A5. Not read.
- **Xu, Tang, Lau, Pan: "A Solid Model Based Virtual Hairy Brush", CGF 21(3), Eurographics 2002** [P, abstract only]. Hair clusters ("writing primitives") with NURBS surfaces that split dynamically. Xu's splitting is heuristic. Not read.
- **Chu, Baxter, Wei, Govindaraju: "Detail-preserving paint modeling for 3D brushes", NPAR 2010.** The PDF fetch failed. **[U]** It uses an imprint or footprint map to add bristle streak detail to a surface brush.
- **DiVerdi et al.: "A lightweight, procedural, vector watercolor painting engine" (I3D 2012) and "Painting with Polygons" (TVCG 2013)** [U, not read]. These are procedural 2D watercolor engines, not bristle physics. Little transfers beyond stroke rendering.
- **Adobe Fresco live brushes** [P, marketing and help pages only]: https://research.adobe.com/news/fresco-the-future-of-painting-born-of-a-union-of-art-and-science. The engine comes from Byungmoon Kim's work with Daichi Ito; Wetbrush is named as an earlier result. Adobe describes it as "pixel-based… physics-driven". **No technical publication found; what bristle model it uses, if any, is unverified.**

---

## Part B: Strand and rod simulation suited to GPU real time

### B1. Bergou, Wardetzky, Robinson, Audoly, Grinspun: "Discrete Elastic Rods", SIGGRAPH 2008 [V, partially read]
- PDF: https://www.cs.columbia.edu/cg/pdfs/143-rods.pdf
- **Centerline:** vertices x₀…x_{n+1}, edges eⁱ. Twist is handled with a Bishop (parallel-transport) frame and the material frame is treated quasi-statically.
- **Discrete curvature binormal:** (κb)ᵢ = 2 e^{i−1}×eⁱ / (|e^{i−1}||eⁱ| + e^{i−1}·eⁱ). Its magnitude is 2 tan(φᵢ/2), so it penalizes kinks.
- **Isotropic bending energy:** E_bend = Σ α (κb)ᵢ² / l̄ᵢ, where l̄ᵢ = |e^{i−1}| + |eⁱ| and α = EI (bending modulus). The anisotropic version uses material curvatures ωᵢʲ = ((κb)ᵢ·m₂ʲ, −(κb)ᵢ·m₁ʲ) and E = Σ (1/2l̄ᵢ) Σⱼ (ω − ω̄)ᵀ B (ω − ω̄), with a rest curvature ω̄.
- **What transfers:** the curvature-binormal measure is the right quantity for "bend toward rest". Comparing material curvatures to rest makes bending rotation-invariant and lets a hair carry a natural curvature. DER's implicit Newton solve is heavier than needed. Twist matters little for round synthetic or sable hairs; only the frame needs care.

### B2. Müller, Heidelberger, Hennix, Ratcliff: PBD (2007), and Macklin, Müller, Chentanez: "XPBD: Position-Based Simulation of Compliant Constrained Dynamics", MIG 2016 [V]
- PDF: https://matthias-research.github.io/pages/publications/XPBD.pdf
- **Problem it fixes:** in PBD, "the effective constraint stiffness is… dependent on both" the time step and the iteration count. A stiffness factor k ∈ [0,1] per iteration is exactly cadmium's "bend toward rest by a stiffness share per iteration".
- **Update:**
  - Δλ = (−C − α̃λ − γ∇C·(x − xⁿ)) / ((1 + γ)∇C M⁻¹∇Cᵀ + α̃)
  - Δx = M⁻¹∇CᵀΔλ
  - α̃ = α/Δt², with α the compliance (inverse stiffness)
  - γ = α̃β̃/Δt, with β̃ = Δt²β
- **Results:** it matches a Newton reference. The 3D results ran Jacobi on a GTX 1070, and contacts used zero compliance.
- **What transfers:** express bend and stretch as compliances with physical units (α_bend = l/(EI) per joint), so the brush feels the same at any substep count. The λ values give normal forces for friction and for a pressure readout.

### B3. Macklin, Storey, Lu, Terdiman, Chentanez, Jeschke, Müller: "Small Steps in Physics Simulation", SCA 2019 [V]
- PDF: https://mmacklin.com/smallsteps.pdf
- **Key claim:** "performing a single large time step with n constraint solver iterations is less effective than computing n smaller time steps, each with a single constraint solver iteration."
- **Loop:** predict x̃ = xⁿ + Δt_s vⁿ + Δt_s² M⁻¹f_ext. Do one pass over the constraints (XPBD Δλ, Δx). Set v = (x − xⁿ)/Δt_s. For example, 30 substeps × 1 iteration takes 13.5 ms against 12.4 ms for 1 × 30, with much less stretch.
- Explicit XPBD damping (eq. 8) becomes necessary because small steps dissipate less.
- **Collision detection is done once per frame** with the full-step predicted trajectory, and the contact set is reused across substeps.
- **What transfers:** replace "substeps chosen by handle travel" with a fixed or budgeted number of substeps (for example 8–32), one Gauss-Seidel sweep per hair per substep, and canvas contact re-evaluated each substep, which is cheap because the canvas is a plane plus a height field.

### B4. Friction and contact in position-based solvers [V]
- **Macklin, Müller, Chentanez, Kim: "Unified Particle Physics for Real-Time Applications", SIGGRAPH 2014, §6.1** (https://mmacklin.com/uppfrta_preprint.pdf).
  - Position-level Coulomb friction after resolving penetration d: Δx⊥ = [(x*ᵢ − xᵢ) − (x*ⱼ − xⱼ)]⊥n.
  - Then Δxᵢ = (wᵢ/(wᵢ + wⱼ)) · { Δx⊥ if |Δx⊥| < μ_s d; else Δx⊥ · min(μ_k d/|Δx⊥|, 1) }.
  - This is static friction at position level, which velocity damping cannot do. Strength is somewhat iteration-dependent.
- **Müller, Macklin, Chentanez, Jeschke, Kim: "Detailed Rigid Body Simulation with Extended Position Based Dynamics", SCA 2020** (https://matthias-research.github.io/pages/publications/PBDBodies.pdf).
  - Static: apply Δx = Δp_t with α = 0, only if λ_t < μ_s λ_n.
  - Dynamic, in a velocity pass after the position solve: Δv = −(v_t/|v_t|) min(h μ_d |f_n|, |v_t|), with f_n = λ_n/h². This is unconditionally stable.
  - Joint damping: Δv = (v₂ − v₁) min(μ_lin h, 1).
- **What transfers:** the static/kinetic friction anchors cadmium has now become standard, solver-consistent rules. The normal force from λ_n makes friction depend on pressure automatically: a light touch gives little drag, a heavy press gives large drag and bending.

### B5. Müller, Kim, Chentanez: "Fast Simulation of Inextensible Hair and Fur" (Dynamic Follow-The-Leader), VRIPHYS 2012 [V]
- PDF: https://matthias-research.github.io/pages/publications/FTLHairFur.pdf
- **FTL:** process vertices root to tip, and move each vertex onto the sphere of radius l₀ around its (already fixed) predecessor. This gives exact inextensibility in one pass. The behaviour is that of a chain where each particle is infinitely heavier than the next.
- **Velocity correction:** vᵢ ← (pᵢ − xᵢ)/Δt + s_damping(−d_{i+1}/Δt), with s_damping ≈ 0.9–1. This hides the uneven mass at the cost of some damping.
- **Hair-hair interaction via a grid:**
  - friction v ← (1 − s_friction)v + s_friction v_grid
  - repulsion v ← v + s_repulsion ∇ρ/|∇ρ|/Δt
- **Cost:** 47k hairs and 776k particles at 25 fps on a GTX 480, including repulsion.
- **What transfers:** a cheap, exact inextensibility pass per hair, which suits a one-thread-per-hair kernel. The grid-velocity smoothing is a cheap model of wet-hair viscous coupling.

### B6. Kugelstadt and Schömer: "Position and Orientation Based Cosserat Rods", SCA 2016 [V]
- PDF: https://animation.rwth-aachen.de/media/papers/2016-SCA-Cosserat-Rods.pdf
- **Setup:** particles plus one unit quaternion per edge (staggered).
- **Constraints:**
  - stretch/shear Γ = (x_{i+1} − xᵢ)/l − d₃(q)
  - bend/twist ΔΩ = Ω − Ω⁰, with the discrete Darboux vector Ω ≈ (2/l) Im(q̄_{i−½} q_{i+½}), measured in the material frame and therefore rotation-invariant
  - |q| = 1 enforced separately
- **Behaviour:** only 2 constraints per element. It handles rest curvature, anisotropic bending and twist. For 50 elements it runs 0.03–0.14 ms per frame on 1 CPU core, 12 to 70 times faster than position-based elastic rods (Umetani et al. 2014). It inherits PBD's step and iteration dependence unless used with XPBD compliance.
- **What transfers:** a cleaner "bend toward rest" that is rotation-invariant (it follows the ferrule as it turns in a curve) and supports a per-hair natural curve or splay (rest Ω⁰). Orientation per edge also gives a frame for flat or filbert ferrules.

### B7. More recent rod and strand solvers
- **Hsu, Wang, Wu, Yuksel: "Stable Cosserat Rods", SIGGRAPH 2025** [V, abstract and introduction]. PDF: https://graphics.cs.utah.edu/research/projects/stable-cosserat-rods/Stable_Cosserat_Rods-SIGGRAPH25.pdf
  - Splits position and rotation optimization, with a closed-form Gauss-Seidel quasi-static orientation update.
  - Stable at high stiffness and large steps, and parallel. The authors report it "orders of magnitude faster and more stable than… XPBD and discrete elastic rods". About 2M DOF ran at 4 fps on an RTX 3090.
  - Relevant if very stiff hog bristles are wanted at few substeps.
- **Deul, Kugelstadt, Weiler, Bender: "Direct Position-Based Solver for Stiff Rods", CGF 2018** [U, not read]. A tree-structured direct solve for stiff rods.
- **Umetani, Schmidt, Stam: "Position-based Elastic Rods", SCA 2014** [P]. PBD rods with ghost particles, the predecessor of B6.

---

## Part C: Wet-hair physics and clumping

### C1. Bico, Roman, Moulin, Boudaoud: "Elastocapillary coalescence in wet hair", Nature 432:690 (2004) [V]
- PDF (author copy): https://blog.espci.fr/jbico/files/2018/04/bico04.pdf (DOI 10.1038/432690a)
- **Setup:** a model brush of parallel elastic lamellae with spacing d, withdrawn from a wetting liquid. The liquid rises to height L_wet between two lamellae; the free "dry" length above is L_dry = L − L_wet.
- **Elastocapillary length:** **L_EC = (κ/γ)^{1/2}**, with κ the bending stiffness and γ the surface tension.
- **Pair sticking (eq. 1):** **L_dry⁴ = (9/2) d² L_EC²**. Capillary energy 2γL_wet is balanced against elastic energy 3κd²/L_dry³; gravity is negligible.
- **Clusters:** a cluster of N lamellae behaves as one lamella N times stiffer, and friction is neglected because the liquid lubricates. Self-similar pairing of N/2 + N/2 clamped at Nd/2 gives eq. 2: **L_dry⁴ = (9/16) N³ d² L_EC²**.
- The maximum cluster size N_max follows from L_dry = L. Clumps are hierarchical (pairs of pairs). Random imperfections give a broad distribution with **mean ≈ 0.67 N_max**. Withdrawal aggregates clumps; the reverse process fragments them.
- **Experimental values:** polyester strips, e = 100 µm, κ = 5.1×10⁻⁴ N·m, d = 1 mm; silicone oil with γ = 20.6 mN/m and ρ = 950 kg/m³. This gives L_EC ≈ 47 mm (e = 50 µm), 150 mm (e = 100 µm) and 370 mm (e = 170 µm).
- **What transfers:**
  - Clumps form a hierarchy of pairs, not one pull toward a single centroid.
  - The clump size is predicted by the hair length, the spacing and L_EC.
  - A clump bends like one thick hair (stiffness × N), so a wet brush reads stiffer and pointier.
  - Clumps release as the liquid leaves, so wetness or loading should control clumping, and drying or exhaustion should split the tip.

### C2. Related elastocapillary work
- **Py, Bastien, Bico, Roman, Boudaoud: "3D aggregation of wet fibers", EPL 77:44005 (2007)** [P, abstract]. Covers cylindrical fibres in 3D bundles: capillary attraction from liquid bridges against the restoring torque, with a cascade of pairings. **The fibre-geometry formula was not read.**
- **Singh, Lister, Vella: "A fluid-mechanical model of elastocapillary coalescence", JFM 2014** (arXiv https://arxiv.org/pdf/1310.4530) [V, introduction].
  - Spring-block elements separated by lubricating films, with N ~ 10⁴ elements.
  - Pairwise coalescence is the most unstable mode. Maximum and mean cluster size scale as **K^{−1/2}**, where K is an elastocapillary number (dimensionless spring stiffness). Local perturbations launch fronts that leave "frozen-in" clusters.
  - A lattice-friendly, GPU-friendly dynamic model of clumping.
- **Hadjittofis, Lister, Singh, Vella: "Evaporation effects in elastocapillary aggregation", JFM 2016** (arXiv 1506.07241) [P]. Evaporation rate changes cluster sizes.
- **Chiodi, Roman, Bico: "Piercing an interface with a brush: collaborative stiffening", EPL 90:44006 (2010)** [V, abstract]. Compressive capillary force at hair tips piercing an interface can buckle them, while bundles resist.
- **Kim and Mahadevan: "Capillary rise between elastic sheets", JFM 2006** [P]. Modified Jurin law for paint-brush-like geometry.
- **Duprat, Protière, Beebe, Stone: "Wetting of flexible fibre arrays", Nature 482:510 (2012)** [P]. There is a critical drop volume above which fibres do not coalesce, and a drop size that maximizes capture. This is relevant to how much paint a brush can hold before it stops pointing.
- **Ushay and Brun, APS March Meeting 2023, "Elastocapillary painting: deposition of fluid trapped within fiber arrays"** [P, talk abstract]. Oil-wetted brushes made of elastic beam arrays, stamping against brushing. Fluid dewetting and transfer depend on drag speed and retraction speed. This is the only physics study found on paint transfer from a fibre brush, and it is preliminary.

### C3. Capillary forces in virtual brushes
- Wetbrush (A8) models thickness and clustering with a minimum-density constraint and models paint-to-bristle adhesion in the sample frame. It has no explicit capillary attraction between bristles; clustering comes from the brush shape and density.
- Chu and Tai (A5): a wetness-proportional lateral stretch spring pulls bristle groups toward the spine.
- No brush paper found implements Bico's L_EC law. **This is a gap, and an opportunity.**

---

## Part D: Material constants

| Quantity | Value | Source / status |
|---|---|---|
| Brush bending stiffness (whole head, per joint) | 5–10 N·mm/rad; head bends about 90° under 1–2 N | Baxter and Lin 2004 [V] (their estimate) |
| Brush-canvas friction μ | 0.3–0.5 | Baxter and Lin 2004 [V] |
| Brush-canvas friction μ (normalized f_N) | 0.75 (f_N = 1), or 0.2 with f_N = 0.7 + 20κ² | Baxter and Govindaraju 2010 [V] |
| Internal friction D (plasticity) | ≤ 0.01 (same units as K) | Baxter and Lin 2004 [V] |
| Plasticity zero-shift cap α | 0–0.10 rad | Chu and Tai 2002 [V] |
| Bend non-linearity exponent m | 3 | Chu and Tai 2002 [V] |
| Inertial coupling β_B | 0–0.1 | Wetbrush [V] |
| Vertices per bristle / samples | 10 / 128 (samples below 1 px apart) | Wetbrush [V] |
| Bristles per brush | 40–600 (Wetbrush); 50–100 (DiVerdi patents) | [V] / [P] |
| Paint reference density ρ₀ | 1000 kg/m³ | Wetbrush [V] |
| Sample capacity compression factor μ | 0.5 | Wetbrush [V] |
| Nylon 6.12 filament flexural modulus | 430–630 ksi ≈ 3.0–4.3 GPa; tapered tip = 0.5–0.75 × butt diameter | US 6,311,359 patent [P] |
| PBT (polyester, "Taklon") vs nylon 6.12 tensile modulus | about 520 vs about 490 kpsi (≈ 3.6 vs 3.4 GPa); PBT keeps stiffness when wet | supplier datasheet via Scribd [U, secondary] |
| Taklon filament diameter | 0.08–0.20 mm (Wikipedia); about 0.04 mm (Gordon Brush blog) | [U, secondary, conflicting] |
| "Bristle" (trade definition) | hair ≥ 0.2 mm diameter; hog bristle tapers and has split ("flagged") tips | da Vinci brush maker page [U, manufacturer] |
| Human hair Young's modulus | 2.0–3.7 GPa (review); 3.7–4.2 GPa (other studies) | [U, search snippets of Meyers-group papers; not opened] |
| Pig hair (hog) Young's modulus | ≈ 6.4 GPa, extensibility ≈ 31% | [U, snippet of a pig-hair tensile study; not opened] |
| Keratin hair density | ≈ 1.3 g/cm³ | [U, patent citing hair literature] |
| Kolinsky sable, squirrel hair diameter | **not found** in primary sources. Kolinsky is "larger near the middle and tapers off to a fine point" (FSU microscopy primer) | [U] |
| Hair friction anisotropy | against-scale higher than with-scale, about 2× ("with-scale about half against-scale") | [U, abstracts of hair tribology papers] |
| Oil paint viscosity (tube) | "> 10,000 cSt" ≈ > 10 Pa·s, shear-thinning | Baxter thesis [V quote; estimate by analogy, U] |
| Oil paint yield stress | ≈ 10–20 Pa for 32 vol% pigment in linseed oil; ≈ 3000 Pa with egg-yolk capillary suspension | Ranquet et al., Nat. Commun. 14:1534 (2023) [V] |
| Linseed oil viscosity | ≈ 50 mPa·s at 20 °C | vendor TDS [U] |
| Linseed oil surface tension | ≈ 33–35 mN/m | **[U, not found in a primary source; recollection]** |
| Linseed oil–water interfacial tension | ≈ 11 mN/m | Ranquet 2023 [V] |

**Derived estimate [U, my own calculation, not from any paper].** Take a synthetic filament with d = 0.1 mm, E = 3.5 GPa, L = 15 mm.
- I = πr⁴/4 ≈ 4.9×10⁻¹⁸ m⁴, so EI ≈ 1.7×10⁻⁸ N·m².
- Single-hair cantilever stiffness 3EI/L³ ≈ 0.015 N/m.
- Baxter's 1–2 N for a 90° bend therefore needs on the order of thousands of real hairs. **The few hundred simulated hairs must each carry N_real/N_sim of the stiffness.**
- Adapting C1 with a fibre bending stiffness and capillary force per length ~γr (Py 2007, not verified): L_EC ~ (E r³/γ)^{1/2} ≈ 10 cm for γ ≈ 33 mN/m, giving N_max ≈ 9 hairs per clump at 0.1 mm spacing. Treat this only as an order of magnitude.

---

## Part E: Synthesis for cadmium (GPU, a few hundred hairs, real-time oil brush)

### Recommended combination
1. **Solver core: XPBD with small steps (B2, B3), one hair per thread or subgroup.**
   - 8–12 vertices per hair. Wetbrush uses 10; Chu and Tai and Baxter-Govindaraju argue for shorter segments toward the tip.
   - Fixed substeps (16–32 per frame), with one Gauss-Seidel sweep root to tip per hair per substep. A few hundred hairs × 10 vertices × 32 substeps is small next to FTL's 776k particles at 25 fps on a GTX 480.
   - Store λ only if forces are needed (normal force for friction, pressure feedback).
   - Replace "stiffness share per iteration" with compliances α = 1/k in physical units. This removes the dependence on step and iteration count named in the XPBD paper.
2. **Inextensibility:** either XPBD distance constraints with α = 0, or one FTL pass (B5) with s_damping ≈ 0.9 after the sweep for exact length.
3. **Bending toward rest, rotation-invariant (fixes "turning in curves" and "spring back"):**
   - Bend constraints on the DER curvature binormal (B1), or Kugelstadt Darboux constraints (B6).
   - Express the rest curvature in a frame transported from the ferrule, not in world space, so rest follows the handle's rotation.
   - Taper stiffness toward the tip: EI ∝ r⁴, with tip/base diameter 0.5–0.75 giving 6–30% of base stiffness. Baxter, Chu-Tai and DiVerdi all make the base stiffer.
   - Optionally make it non-linear, E ∝ |θ|³ (Chu and Tai), so small bends are soft and large bends are firm.
   - Twist can be dropped for round brushes (Baxter, DiVerdi). Keep it only for flats if needed.
4. **Brush-frame integration (A8, eq. 2, and A7):** integrate in the ferrule frame and scale inertial pseudo-forces by β_B ∈ [0, 0.1]. Small β_B keeps hairs with fast handles (no lag explosions) yet gives a little flick and overshoot on spring-back. DAB's "Aristotelian" observation says real hairs are overdamped, so keep β_B small and add XPBD damping.
5. **Touch and pressure (fixes first contact and light touch):**
   - Map pressure to handle height or penetration, not force: p = 0 means the tip just touches (h_min), p = 1 means the maximum press (A7, eqs. 1–3). Clamp by stiffness so stiff brushes press in less.
   - First contact then comes from geometry, because the point lands first.
   - For realistic buckling (A1), give each hair a small random rest curvature or splay (rest Ω⁰ or rest κb noise) so a near-vertical press buckles in a consistent direction rather than balancing.
   - Use λ_n from the canvas constraint as the actual contact force.
6. **Contact and friction:**
   - The canvas is a unilateral plane or height-field constraint with a per-vertex radius (Chu-Tai effective radii).
   - Static friction at position level when λ_t < μ_s λ_n; dynamic friction in a velocity pass, Δv = −v̂_t min(hμ_d f_n, |v_t|) (B4).
   - Add Baxter's anisotropic factor (1 − C_η max(0, d_pull·Δx̂)^k) so a pushed tip catches and a pulled tip glides, or Chu-Tai's κ_f parallel/perpendicular split.
   - Optionally add Chu-Tai's blocking-plane "pore resistance" for pushed strokes on rough canvas.
   - Hysteresis between stick and slip (Baxter 3.4.1).
   - Starting values: μ_s ≈ 0.5, μ_k ≈ 0.3, C_η ≈ 0.5.
7. **Plasticity and memory across strokes:** do not reset the brush each stroke. Keep the rest shape and shift it toward the current shape with Chu-Tai's zero-shift (ρ = min(θ′, α), α up to about 0.1 rad, scaled by wetness), or Baxter's joint friction. Add an explicit "reshape" action, the analogue of a painter re-pointing a brush.
8. **Clumping (wet) and splitting (dry), grounded in C1:**
   - Replace "pull to centroid + summed spacing push" with two constraints:
     - (a) a unilateral minimum-distance or minimum-density constraint for hair thickness (Wetbrush);
     - (b) compliant attraction constraints between hairs in the same clump.
   - Build clumps hierarchically by pairing neighbours (Bico: pairs of pairs). Size them with N_max from eq. 2, using an effective L_EC that grows with paint load and wetness, and aim for a mean ≈ 0.67 N_max.
   - As load drops (dry brush), shrink L_EC so clumps split. This produces flying-white and dry-brush gaps (Chu-Tai's observation).
   - A clump can be given N× bending stiffness, so a loaded brush reads as stiffer and pointed.
   - The Singh-Lister-Vella K^{−1/2} scaling gives one tunable knob.
9. **The mark left:**
   - Sample each hair as a Hermite spline at sub-pixel spacing (Wetbrush, 128 samples) and deposit per sample.
   - Give each sample a load m_j and capacity M_j that falls with local bristle density and canvas proximity (Wetbrush eqs. 12–13). Pressing squeezes paint out, light touches deposit only from the tip samples, and exhausted samples leave gaps. That gives streaks, ragged edges and dry-brush in one mechanism.
   - The Bingham numbers (yield stress about 10–20 Pa, viscosity about 10 Pa·s or more) suit a 2.5D paint layer that only flows where the bristle shear exceeds the yield stress, in the spirit of Baxter's three-regime model.

### Numbers to adopt first
- Substeps 16–32, 1 iteration each, compliance-based.
- Vertices per hair: 10. β_B = 0.05.
- μ_s = 0.5, μ_k = 0.3; anisotropy C_η = 0.5, k = 2.
- Plastic shift α ≤ 0.1 rad; tip/base diameter 0.6; bend exponent 2–3.
- Whole-head stiffness calibrated so that 1–2 N bends the head about 90°.
- ρ₀ = 1000 kg/m³; capacity compression μ = 0.5.
- Paint yield stress about 15 Pa.

### Biggest gaps and unverified items
- DiVerdi et al. 2010 (VRST) full text; only the abstract, Wetbrush's description and patents were seen.
- Hadap 2006, Chu-Tai 2004 CG&A, Xu 2002, Saito 1999 and Chu 2010 were not read.
- Measured diameters for sable, squirrel and hog hair; numeric hair friction coefficients; linseed oil surface tension; a primary oil-paint viscosity.
- How Fresco live brushes work.
- The fibre-geometry elastocapillary formula (Py 2007).
