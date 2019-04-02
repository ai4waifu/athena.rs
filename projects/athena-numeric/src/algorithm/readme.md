# Algorithms and formulas

This is the maintainer's map of the exact arithmetic implementation. It is intentionally close to the Rust code: every
section names the implementing function, states the representation invariant, derives the transformation, and explains
the crossover costs. Symbols use Unicode math notation so the source remains readable in rustdoc and ordinary editors.

## Limb representation

A non-negative integer is stored in radix β = 2⁶⁴:

```text
x = x₀ + x₁β + ⋯ + xₙ₋₁βⁿ⁻¹,    0 ≤ xᵢ < β.
```

Limbs are little-endian. A non-zero value has a non-zero last limb. Zero is represented canonically, never by an empty
public magnitude. `Integer` adds a sign bit and forbids negative zero. Any algorithm returning limbs must restore this
canonical form before the value crosses the kernel boundary.

### Measured cost boundary

The arithmetic kernel must not be blamed for runtime-lifetime costs. A release build measurement at four limbs found:
stack addition ≈1 ns, isolated `Natural::try_add` with a disabled heap ≈253 ns, while an
`Integer` clone on `GcHeap::shared_default()` with automatic collection was ≈4,035 ns. The public `Integer::try_add`/
`add` path measured ≈4,503/4,669 ns. Therefore a 256-bit plateau in an end-to-end benchmark is a shared-heap and
clone/publish effect, not a four-limb addition algorithm. Benchmarks must report kernel, reused-context numeric, and e2e
layers separately.

`Integer::abs_natural` currently owns a cleared-sign magnitude. For a heap magnitude that ownership step clones the
allocation. Any optimization must first provide a borrowed magnitude view or a session-owned/deferred result policy;
changing a multiplication formula cannot remove this cost.

## Addition and subtraction

For addition, limb i computes

```text
t = aᵢ + bᵢ + carryᵢ
outᵢ = t mod β
carryᵢ₊₁ = ⌊t / β⌋.
```

`u128` represents t without overflow. Subtraction is the analogous borrow recurrence and requires a ≥ b. Both algorithms
are Θ (max (m,n)) and need only the output buffer. See `add_slices_into`, `sub_slices_into`, `adc`, and `sbb`.

## Schoolbook multiplication

Expanding positional notation gives

```text
(Σ aᵢβⁱ)(Σ bⱼβʲ) = Σᵢ Σⱼ (aᵢbⱼ)βⁱ⁺ʲ.
```

Therefore each aᵢbⱼ is accumulated at output position i+j, with carry propagated in radix β. This is Θ (mn), has small
setup cost, and is the correct choice for short or strongly unbalanced inputs. See `mul_schoolbook_into`.

## Karatsuba multiplication

Split both operands at k limbs:

```text
a = a₀ + a₁βᵏ                  b = b₀ + b₁βᵏ
z₀ = a₀b₀                      z₂ = a₁b₁
z₁ = (a₀+a₁)(b₀+b₁) − z₀ − z₂
ab = z₀ + z₁βᵏ + z₂β²ᵏ.
```

The identity replaces four half-size multiplications by three. Its recurrence T (n)=3T (⌈n/2⌉)+Θ (n) gives Θ (nˡᵒᵍ²³).
It is slower on small inputs because it additionally allocates scratch, forms two sums, performs two subtractions,
recursively dispatches three times, and combines shifted results. Those linear costs dominate until a multiplication is
wide enough. Unbalanced inputs also waste recursive work after zero-padding, so the planner may keep schoolbook
multiplication. See `mul_rec` and `karatsuba_scratch_limbs`.

Preconditions: scratch must satisfy the planner's bound, output must hold m+n limbs, and the subtraction order above is
safe because z₁ is exactly the sum of the two non-negative cross products a₀b₁+a₁b₀.

## Toom–3 multiplication

Toom–3 treats each operand as a degree-two polynomial in X = βᵏ:

```text
A(X)=a₀+a₁X+a₂X²              B(X)=b₀+b₁X+b₂X².
```

Their product has degree four, hence five independent values determine it. Evaluate A and B at 0, 1, −1, 2, and ∞,
multiply pointwise, interpolate the five coefficients, then substitute X=βᵏ. The ∞ value means the leading coefficient:
A (∞)B (∞)=a₂b₂. Exact divisions by 2 and 3 appear during interpolation because the evaluation matrix has those factors.
A non-exact division indicates an implementation error, not rounding.

Five multiplications of about n/3 limbs replace nine block products, giving T (n)=5T (⌈n/3⌉)+Θ (n)=Θ (nˡᵒᵍ³⁵). The
asymptotic improvement is real, but each call also performs ten evaluations, signed intermediate arithmetic, exact small
divisions, interpolation, and recomposition. For small inputs these operations cost more than the four schoolbook
products they save. Unequal block lengths and leading zero blocks amplify that overhead. This is why the planner uses a
measured crossover rather than selecting Toom–3 merely because both operands are heap values. See `toom3_mul_rec`,
`split_three`, and
`toom3_scratch_limbs`.

Boundaries: signed evaluation at −1 must not be stored as an unsigned magnitude, interpolation temporaries need one or
more guard limbs, and every exact division must check its remainder. Aliasing output with an input is not permitted
unless a wrapper explicitly copies that input first.

## Multi-limb division: Knuth Algorithm D

For u ÷ v, first left-shift both numbers so the highest bit of v is set. This makes the leading limb a reliable divisor
approximation. At position j, estimate q̂ from the leading two limbs of the current dividend and the leading limb of v,
cap q̂ below β, then test it against the next limb. Subtract q̂v. If subtraction borrows, q̂ was one too large:
decrement it and add v back. Finally right-shift the remainder by the normalization count.

The correction proof follows from the normalized leading-limb bound: the two-limb estimate can exceed the true quotient
digit by only a small amount, and the second-limb test reduces this to at most one before subtraction. Postconditions
are u=qv+r and 0≤r<v. Division by zero is rejected before the kernel. See `div_rem_knuth_into`, `shl_into`, and
`shr_into`.

## GCD and Lehmer acceleration

Euclid uses gcd (a,b)=gcd (b,a mod b), because common divisors are unchanged by replacing a with a−qb. Full multi-limb
division is expensive. Lehmer observes that, while a and b have similar widths, their leading limbs often determine
several successive Euclidean quotients. It accumulates the corresponding 2×2 integer matrix and applies that matrix once
to the full operands.

A candidate matrix is accepted only while its quotient predictions remain stable for the lower and upper leading-limb
bounds and its signed linear combinations stay representable and non-negative. Otherwise the code falls back to one
exact Euclidean remainder step. Progress requires the second operand to decrease. See `gcd`, `lehmer_step`, and
`lincomb_signed`.

Binary GCD is a separate baseline: remove common powers of two, repeatedly subtract the smaller odd magnitude from the
larger, remove new powers of two, then restore the common shift. See `binary_gcd`.

## Montgomery modular multiplication

Let m be odd and k its limb width. Choose R=βᵏ, so gcd (R,m)=1, and compute m′ such that mm′≡−1 (mod β). Montgomery
representation stores x̄=xR mod m. Given T=x̄ȳ, reduction repeatedly chooses uᵢ=Tᵢm′ mod β. Then T+uᵢm has zero limb i,
so division by β is a limb shift rather than a general division. After k steps the result is congruent to TR⁻¹ mod m and
lies below 2m; one conditional subtraction produces the canonical residue.

The method requires odd m. For even m, R has no inverse modulo m, so this representation is invalid and the caller must
use the ordinary division path. Setup computes R² mod m and m′; it is profitable only when reused across many
multiplications, as in modular exponentiation. See `montgomery_nprime`,
`montgomery_redc`, `montgomery_precompute`, and `mul_mod_mont_with`.

## Exponentiation

Binary exponentiation follows e=Σ eᵢ2ⁱ. Starting with acc=1 and base=a, scan bits from least significant to most
significant: multiply acc by base when eᵢ=1, square base, then shift e. It needs ⌊log₂e⌋ squarings and popcount (e)
general multiplications. Modular exponentiation performs the same schedule in Montgomery form so intermediate values
remain bounded by m.

## Rational cross-cancellation

For (a/b)(c/d), direct multiplication may create huge intermediates that are immediately cancelled. Compute g₁=gcd
(|a|,d) and g₂=gcd (|c|,b), then

```text
(a/b)(c/d) = ((a/g₁)(c/g₂)) / ((b/g₂)(d/g₁)).
```

This is identical because each g divides one numerator and the opposite denominator. It preserves a positive denominator
and substantially limits intermediate growth. See `cross_cancel_mul_ctx`.

## References

- D. E. Knuth, *The Art of Computer Programming, Volume 2*, §§4.3.1–4.3.3.
- R. P. Brent and P. Zimmermann, *Modern Computer Arithmetic*, Chapters 1–2.
- A. Karatsuba and Y. Ofman, “Multiplication of Multidigit Numbers on Automata”.
- A. L. Toom, “The Complexity of a Scheme of Functional Elements Realizing the Multiplication of Integers”.
- D. H. Lehmer, “Euclid's Algorithm for Large Numbers”.
- P. L. Montgomery, “Modular Multiplication without Trial Division”.
