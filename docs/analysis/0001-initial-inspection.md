# Mathcad Prime 12 최초 분석

조사일: 2026-10-15. 실제 제품 코드를 실행하지 않고 설치 모듈과 저장 문서를 정적으로 조사했다.

## 도구와 입력

- REA 6.5.0 / Node.js 24.13.1.
- Ghidra 12.1.4 / Eclipse Temurin JDK 21.0.12.1+1을 별도 사용자 도구 경로에 설치하고 REA 제공자 사전 검사를 통과했다.
- 실제 설치 경로: `C:\Program Files\PTC\Mathcad Prime 12.0.0.1`.
- 설치 도우미: `pim_installmgr_mathcad.exe`, SHA256 `1690e359f68503dc4958beabb33cfc70e888f78eca2de07e78ccc5f5faa4b2d9`.
- 문서 표본: `C:\Users\harry\Downloads\detune.mcdx`, 48,067 bytes, SHA256 `bd4b329ce3cb2de848843c80e3bd42570022dc54a2c7d335f898c234151c1cbf`.
- 원본 문서, 제품 바이너리, REA의 상세 출력은 Git에 포함하지 않는다. 상세 출력은 무시된 `artifacts/rea/`에 저장했다.

## 설치 모듈에서 관찰한 사실

REA `inspect-managed-artifact`로 우선 모듈 16개를 검사했다. 증거와 해시는 `artifacts/rea/mathcad/*.artifact.json`, 모듈 요약은 `module-summary.json`에 있다.

| 모듈 | 정적 검사 결과 |
| --- | --- |
| `MathcadPrime.exe` | x64 PE, CLI 메타데이터 없음 |
| `MathcadPrime.dll` | managed assembly |
| `Ptc.Core.dll` | .NET 8 CIL, AnyCPU |
| `Ptc.Calculation.Core.dll` | managed assembly, 결과·오류·계산 대상·단위 관련 타입 존재 |
| `Ptc.Calculation.dll` | .NET 8 x64 CIL, 계산 제어부·엔진 생성 타입 존재 |
| `mceng.dll`, `mcdepman.dll`, `eficore_.dll` | x64 C++/CLI 혼합 코드. CIL 검사만으로 네이티브 메서드 본문을 복원할 수 없음 |
| `MCDXSerializer.dll` | managed assembly, 패키지 직렬화 관련 타입 존재 |

PE machine이 x86인 CIL 파일도 AnyCPU일 수 있으므로 PE 헤더의 아키텍처와 managed architecture를 구분한다. `MathcadPrime.runtimeconfig.json`에는 .NET 8 및 WindowsDesktop 프레임워크가 선언되어 있다.

## 계산 경로의 첫 연결

REA `inspect-managed-members` 결과에서 다음 정적 호출을 확인했다. 호출 존재는 관찰 사실이며 실제 실행 순서·조건 전체를 의미하지 않는다.

- `Ptc.Calculation.EngineBuilder.CreateEngine` (`0x06000056`) → `Engine.IFactory.NewEngine`, `Engine.IEngine.SetSynchronousCallbacks`, `SetOption`, `AddAfter`.
- `Ptc.Calculation.CalculationController.HandleCalculateNow` (`0x06000016`) → `Engine.IEngine.CalculateNow`, `HandleStartEvaluating`.
- `Ptc.Calculation.Core`에 `EngineResult`, `EngineError`, `EngineRedefInfo`, `ICalculatable`, `ICalcWithCache` 타입이 존재한다.
- `mceng.dll`에 `Engine.FactoryImp`, `mcdepman.dll`에 `depman.DependencyType` 및 위치·정의·액션 체인 관련 C++ 타입명이 존재한다.

계산 제어부에서 엔진 인터페이스를 호출하는 구조가 드러났다. `mcdepman`의 정확한 평가 순서·재정의 규칙은 아직 미확인이다. 타입명만으로 알고리즘을 확정하지 않는다.

메서드·정적 호출 수는 검사 범위 내에서 `Ptc.Calculation.Core` 828/1,975, `Ptc.Calculation` 268/1,253, `mceng` 166/444, `mcdepman` 1,127/3,481, `MCDXSerializer` 63/241이다. 혼합 코드의 네이티브 본문까지 분석했다는 의미는 아니다.

## MCDX 구조

REA `inspect-artifact`가 표본을 ZIP으로 인식했다. Evidence ID: `ev_9237cf3bd733e97b1f2e81e138de522101593b43b660f738ebdc89a0060f059d`. ZIP 항목 목록은 REA 증거이고, 아래 XML 세부 관찰은 별도 PowerShell XML 검사 결과다.

- 패키지 항목 33개, `mathcad/worksheet.xml`에 영역 87개: 수식 66개, 텍스트 21개.
- 수식은 `http://schemas.mathsoft.com/math50` 네임스페이스의 `define`, `apply`, `eval`, `id`, `real`, `mult`, `div`, `pow` 같은 구조로 저장된다.
- 영역에 `region-id`, `left`, `top` 등 배치 속성이 있으며 수식의 `resultRef`와 별도 결과 데이터가 존재한다.
- `mathcad/result.xml`의 `resultData` 66개는 저장된 캐시다. 이번 조사에서 재계산하거나 정답으로 검증하지 않았다.
- 단위 결과에 `unitedValue`, `unitMonomial`, `unitReference`, 지수의 분자·분모 속성이 관찰된다.
- 텍스트 영역은 관계 ID로 XAML 패키지를 참조한다.
- 설정에는 배열 시작 인덱스 0, 자동 재계산 true, SI 단위계, tolerance 0.001이 기록되어 있다. 이는 이 표본의 설정이며 모든 문서의 기본값으로 일반화하지 않는다.
- 표본의 app 속성은 앱 12.0.0.1, 엔진 12.0.0.21, 직렬화 12.0.0.0을 기록한다. 해당 값은 파일이 선언한 메타데이터다.

`MCDXSerializer`의 `PackageSerializable.Read` / `Write` 타입·메서드와 `/docProps/app.xml`, `/mathcad/settings/calculation.xml`, `/mathcad/settings/presentation.xml` 문자열도 확인했다. 문서에서 발견한 패키지 경로와 일치하지만 전체 파일 형식 명세는 아직 아니다.

## 재현

```powershell
./scripts/inspect-mathcad.ps1
./scripts/inspect-mcdx.ps1 -Path 'C:\Users\harry\Downloads\detune.mcdx'
rea inspect-managed-members 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.Calculation.dll' --json
```

## Windows 네이티브 분석 제한

`mceng.dll`의 Ghidra 분석은 Windows P0의 `managed_target_unsupported` 제한으로 거부됐다. 원본은 변경하지 않았으며 CIL 분석 증거는 유효하다. 혼합 DLL 네이티브 본문은 지원되는 호스트/제공자에서 별도로 조사해야 한다. 거부 응답은 `artifacts/rea/mathcad/mceng-native.json`에 보관했다.

반면 `MathcadPrime.exe`의 Ghidra 분석은 성공했다. SHA256 `0b0c2bcd1997fe884b5ae429ee31b0d44f22896d65c5102b441985bb863f8e76`, Evidence ID `ev_c1671905b5dadee9a8d35026b29ac79920696e6ab6a71021d689cfabbaa59b3c`. 이미지 기준 주소 `0x140000000`, Ghidra에서 관찰한 procedure 569개·문자열 499개·segment 9개다. apphost PDB 참조가 관찰되며 이 런처 분석을 계산 엔진의 알고리즘 분석으로 해석하지 않는다. 결과는 `artifacts/rea/mathcad/launcher-native.json`에 있다.

## 다음 조사

1. `Engine.IFactory.NewEngine`과 `Engine.IEngine.CalculateNow`의 구현 경계 추적.
2. 네이티브 분석으로 `mcdepman`의 위치·정의·의존성 처리 후보 조사.
3. 원본 앱에서 작은 워크시트를 만들어 전방 참조·재정의·영역 배치별 동작 비교.
4. 표본의 저장 결과를 원본 실행 결과와 대조한 뒤 Rust 테스트 사례로 채택.

현재는 M0 조사 진행 중이며 계산 동작 호환성이 입증된 상태는 아니다.
