# REA 분석 환경

2026-10-15 이 Windows PC에 REA 6.5.0을 설치했다. Node.js 24.13.1을 사용한다.

- CLI: `rea --version`, `rea --help`.
- Codex MCP 등록 완료. REA가 생성한 설정 백업은 `C:\Users\harry\.codex\config.toml.rea.backup`에 있다.
- REA workflow skill: `C:\Users\harry\.agents\skills\reverse-engineer-anything`.
- 현재 대화에서는 CLI로 분석했다. MCP 도구를 새 세션에 표시하려면 Codex를 재시작하거나 MCP를 다시 연결한다.
- Ghidra 12.1.4: `C:\Users\harry\.local\share\mathcraft-tools\ghidra_12.1.4_PUBLIC`.
- Eclipse Temurin JDK 21.0.12.1+1: 같은 tools 폴더의 `jdk-21.0.12.1+1`.
- 다운로드한 두 ZIP은 공식 배포 메타데이터의 SHA256과 대조했다. 경로와 SHA256은 `scripts/setup-rea-native.ps1`에 고정했다.
- `MathcadPrime.exe`의 Ghidra 정적 분석 실행도 성공했다. 계산 엔진 네이티브 본문의 지원 여부는 별개다.
- MCP 등록에 Ghidra 및 Java 경로를 반영했다. CLI는 아래 환경 변수를 설정한 터미널에서 사용한다.

```powershell
$env:GHIDRA_INSTALL_DIR = "$env:USERPROFILE\.local\share\mathcraft-tools\ghidra_12.1.4_PUBLIC"
$env:JAVA_HOME = "$env:USERPROFILE\.local\share\mathcraft-tools\jdk-21.0.12.1+1"
rea doctor --provider ghidra --json
```

## 분석 시작

```powershell
./scripts/inspect-mathcad.ps1
./scripts/inspect-mcdx.ps1 -Path 'C:\Users\harry\Downloads\detune.mcdx'
rea inspect-managed-members 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.Calculation.dll' --json
```

`artifacts/rea/`는 Git에서 제외된 로컬 증거 저장소다. 설치 폴더를 수정하거나 원본 제품 DLL을 프로젝트에 복사하지 않는다.

Windows Ghidra P0는 순수 네이티브 PE를 지원하지만 `mceng.dll` 같은 managed/C++/CLI 혼합 DLL은 `managed_target_unsupported`로 거부한다. 이 제한은 설치 실패가 아니다. 혼합 DLL의 CIL은 REA .NET 정적 검사로 조사하고, 네이티브 본문은 지원되는 다른 호스트/제공자에서 별도로 분석해야 한다. 원본 제품 실행 검증과 정적 호출 그래프를 구분한다.

첫 결과는 [최초 분석 기록](analysis/0001-initial-inspection.md)에 정리했다.
