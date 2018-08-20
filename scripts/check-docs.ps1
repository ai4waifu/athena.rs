param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot ".."))
)

$ErrorActionPreference = "Stop"
$readme = Get-Content (Join-Path $Root "README.md") -Raw
$forbidden = @(
    "first run benchmarks CPU matmul",
    "Later runs reuse that choice",
    "天然高于 PyTorch",
    "天然高于 PyTorch/vLLM/sglang",
    "network collectives" 
)
foreach ($phrase in $forbidden) {
    if ($readme.Contains($phrase)) {
        throw "README contains a forbidden unverified claim: $phrase"
    }
}

$matrix = Join-Path $Root "docs/backend-support.generated.md"
if (-not (Test-Path $matrix)) { throw "Missing generated backend support matrix" }
$matrixText = Get-Content $matrix -Raw
if (-not $matrixText.Contains("Generated-artifact contract")) {
    throw "Backend support matrix is missing generated-artifact disclaimer"
}
Write-Output "documentation policy checks passed"
