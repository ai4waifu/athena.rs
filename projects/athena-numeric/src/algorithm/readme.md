# 算法与公式

本文是精确算术实现的维护者地图，刻意贴近 Rust 源码：每一节点名实现函数、陈述表示不变量、推导变换，并说明 crossover 成本。符号用
Unicode 数学记号，以便在 rustdoc 与普通编辑器中可读。

## Limb 表示

非负整数按基 β = 2⁶⁴ 存放：

```text
x = x₀ + x₁β + ⋯ + xₙ₋₁βⁿ⁻¹,    0 ≤ xᵢ < β.
```

Limb 为小端。非零值的最高 limb 非零。零有规范表示，公共 magnitude 从不为空切片。`Integer` 另加符号位，并禁止负零。任何返回
limb 的算法在跨越内核边界前必须恢复该规范形。

### 实测成本边界

不得把运行时生命周期开销算到算术内核头上。四 limb 的 release 实测：栈加法约 1 ns；禁用堆时孤立的 `Natural::try_add` 约 253
ns；而在 `GcHeap::shared_default()` 上带自动回收的 `Integer` 克隆约 4,035 ns。公开的 `Integer::try_add` / `add` 路径约
4,503 / 4,669 ns。因此端到端基准里的 256 位平台期是共享堆与克隆/发布效应，不是四 limb 加法算法。基准须分层报告：内核、复用上下文的数值层、端到端。

`Integer` 热路径应走 `as_limbs` / `magnitude_view`，不要经 owning 清符号复制。`try_abs_natural` /
`magnitude` 仅供确需 `Natural` 所有权的路径。对堆上 magnitude，owning 复制会分配。任何优化须先提供借用视图或会话
持有/推迟结果策略；改乘法公式消不掉这笔成本。

## 加法与减法

加法在 limb i 上计算

```text
t = aᵢ + bᵢ + carryᵢ
outᵢ = t mod β
carryᵢ₊₁ = ⌊t / β⌋.
```

用 `u128` 表示 t 以免溢出。减法是对应的借位递推，并要求 a ≥ b。两者均为 Θ (max (m,n))，只需输出缓冲。见 `add_slices_into`、
`sub_slices_into`、`adc`、`sbb`。

## 课本乘法

展开位值记号得

```text
(Σ aᵢβⁱ)(Σ bⱼβʲ) = Σᵢ Σⱼ (aᵢbⱼ)βⁱ⁺ʲ.
```

因此每个 aᵢbⱼ 累加到输出位置 i+j，并在基 β 上传播进位。复杂度 Θ (mn)，启动成本小，适合短操作数或严重不平衡输入。见
`mul_schoolbook_into`。

## Karatsuba 乘法

将两操作数在 k limb 处切开：

```text
a = a₀ + a₁βᵏ                  b = b₀ + b₁βᵏ
z₀ = a₀b₀                      z₂ = a₁b₁
z₁ = (a₀+a₁)(b₀+b₁) − z₀ − z₂
ab = z₀ + z₁βᵏ + z₂β²ᵏ.
```

该恒等式用三次半长乘法代替四次。递推 T (n)=3T (⌈n/2⌉)+Θ (n) 给出 Θ (n^{log₂3})。小输入更慢，因为还要分配
scratch、做两次求和、两次减法、三次递归调度与移位合并。这些线性成本在乘法足够宽之前占主导。不平衡输入在零填充后也会浪费递归工作，规划器可能继续用课本乘法。见
`mul_rec` 与 `karatsuba_scratch_limbs`。

前置条件：scratch 须满足规划器上界，输出须能容纳 m+n limb，且上述减法顺序安全，因为 z₁ 恰好是两非负交叉积 a₀b₁+a₁b₀ 之和。

## Toom–3 乘法

Toom–3 把每个操作数看作 X = βᵏ 上的二次多项式：

```text
A(X)=a₀+a₁X+a₂X²              B(X)=b₀+b₁X+b₂X².
```

乘积为四次，故需五个独立值。在 0、1、−1、2、∞ 处求值 A 与 B，点乘，插值五个系数，再代入 X=βᵏ。∞ 处表示首项系数：A (∞)B (∞)
=a₂b₂。插值中出现对 2、3 的整除，因求值矩阵含这些因子。非整除表示实现错误，而非舍入。

约 n/3 limb 的五次乘法代替九次分块积，得 T (n)=5T (⌈n/3⌉)+Θ (n)=Θ (n^{log₃5})
。渐近改进真实，但每次调用还有十次求值、带符号中间算术、精确小除法、插值与重组。对小输入这些开销超过省下的四次课本积。块长不等与前导零块会放大开销。因此规划器用实测
crossover，而不是仅因两操作数在堆上就选 Toom–3。见 `toom3_mul_rec`、`split_three`、`toom3_scratch_limbs`。

边界：−1 处的带符号求值不得存为无符号 magnitude；插值临时量需要一或多个保护 limb；每次精确除法须检查余数。输出与输入别名不允许，除非包装层先显式复制该输入。

## 多 limb 除法：Knuth 算法 D

对 u ÷ v，先左移两数使 v 的最高位置 1。这样最高 limb 可作为可靠的商近似除数。在位置 j，用当前被除数的前两 limb 与 v 的最高
limb 估计 q̂，将 q̂ 限制在 β 以下，再与下一 limb 校验。减去 q̂v。若减法发生借位，则 q̂ 偏大一：减一并把 v 加回。最后按规范化移位量右移余数。

校正证明来自规范化最高 limb 界：两 limb 估计最多略大于真商数字，第二 limb 检验可在减法前把偏差压到至多一。后置条件为 u=qv+r
且 0≤r<v。除零在进内核前拒绝。见 `div_rem_knuth_into`、`shl_into`、`shr_into`。

## GCD 与 Lehmer 加速

欧几里得用 gcd (a,b)=gcd (b,a mod b)，因为用 a−qb 替换 a 不改变公约数。完整多 limb 除法昂贵。Lehmer 观察到：当 a、b 宽度相近时，其最高
limb 往往决定若干步欧几里得商。它累积对应的 2×2 整数矩阵，再对该矩阵一次作用于完整操作数。

候选矩阵仅在商预测对最高 limb 上下界保持稳定、且带符号线性组合可表示且非负时被接受。否则回退到一次精确欧几里得余数步。进展要求第二操作数减小。见
`gcd`、`lehmer_step`、`lincomb_signed`。

二进制 GCD 是另一基线：去掉公共 2 的幂，反复从较大奇 magnitude 减去较小者，再去掉新的 2 的幂，最后恢复公共移位。见
`binary_gcd`。

## Montgomery 模乘

设 m 为奇数，k 为其 limb 宽。取 R=βᵏ，则 gcd (R,m)=1，并求 m′ 使 mm′≡−1 (mod β)。Montgomery 表示存 x̄=xR mod m。给定
T=x̄ȳ，约化反复取 uᵢ=Tᵢm′ mod β。于是 T+uᵢm 在 limb i 为零，除以 β 变成 limb 移位而非一般除法。k 步后结果同余于 TR⁻¹ mod m
且小于 2m；一次条件减法得到规范剩余。

该方法要求 m 为奇。对偶 m，R 在模 m 下无逆，表示无效，调用方须走普通除法。预计算 R² mod m 与 m′；仅在多次乘法复用时合算，如模幂。见
`montgomery_nprime`、`montgomery_redc`、`montgomery_precompute`、`mul_mod_mont_with`。

## 幂运算

二进制幂按 e=Σ eᵢ2ⁱ。从 acc=1、base=a 起，自低位向高位扫描：eᵢ=1 时用 base 乘 acc，再平方 base，然后右移 e。需要 ⌊log₂e⌋ 次平方与
popcount (e) 次一般乘法。模幂在 Montgomery 形式下执行同一日程，使中间值受 m 约束。

## 有理数交叉约分

对 (a/b)(c/d)，直接相乘可能产生立刻被约掉的巨大中间量。计算 g₁=gcd (|a|,d) 与 g₂=gcd (|c|,b)，则

```text
(a/b)(c/d) = ((a/g₁)(c/g₂)) / ((b/g₂)(d/g₁)).
```

恒等成立，因为每个 g 整除一个分子与对角分母。保持正分母，并显著限制中间增长。见 `cross_cancel_mul_ctx`。

## 参考文献

- D. E. Knuth, *The Art of Computer Programming, Volume 2*, §§4.3.1–4.3.3.
- R. P. Brent and P. Zimmermann, *Modern Computer Arithmetic*, Chapters 1–2.
- A. Karatsuba and Y. Ofman, “Multiplication of Multidigit Numbers on Automata”.
- A. L. Toom, “The Complexity of a Scheme of Functional Elements Realizing the Multiplication of Integers”.
- D. H. Lehmer, “Euclid's Algorithm for Large Numbers”.
- P. L. Montgomery, “Modular Multiplication without Trial Division”.
