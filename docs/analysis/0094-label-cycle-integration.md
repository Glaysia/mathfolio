# 식별자 라벨 순환·자동 복원과 None 저장

2026-10-19. [0093 원본 계약](0093-label-cycle-contract.md)에 따라 Ctrl+Q와 리본의 자동 라벨을 편집·계산·저장에 연결했다. 원본 enum·HasNoLabel·writer·RL suffix의 정적 근거를 사용했고 GUI·스크린샷·원본 프로세스·어셈블리 실행은 사용하지 않았다.

## 입력과 원본 상태

`Editor::cycle_identifier_label`은 Variable→Unit→Constant→Function→System→Keyword→None→Variable을 따른다. 명시 라벨이 없거나 contextual인 이름은 resolved Unit 등과 관계없이 Variable부터 시작한다. 문자 선택은 그 문자의 글꼴 대신 식별자 전체 라벨을 바꾼다. 단일 Identifier subtree를 지원하며 여러 노드·행렬 셀 범위·비식별자는 변경하지 않는다.

공통 `set_identifier_label` API로 리본과 키 입력을 연결했다. 이름·literal suffix·UTF-16 caret/문자 선택·단일 노드 scope를 보존하고 변경 하나를 Undo 하나로 기록한다. 같은 명시 라벨을 다시 지정하면 이력·redo를 바꾸지 않는다. 실제 명시 선택에서는 contextual attribute를 제거한다. 알 수 없는 명시 라벨을 임의로 분류하지 않는다.

None은 `labels="*"`이며 contextual true와 다른 저장 상태다. 원본 reader가 허용하는 대문자 `NONE`도 동일한 값으로 순환·계산한다. 그대로 읽고 쓰는 raw tree는 해당 원문을 보존하며 사용자가 None을 지정하면 원본 writer의 `*`를 쓴다. 임의 소문자 변형은 추가하지 않았다. 원본 Ctrl+Q의 Typeover 모드와 override flag를 true로 만드는 호스트는 아직 재현하지 않았으며 전체 navigator·read-only 조합의 동등성도 미검증이다.

## 계산과 단위 이름 충돌

원본 `GetRl`은 nullable null과 None6을 같은 label fallback으로 보낸다. 기존 계산기가 `*`를 별도 Symbol namespace로 취급하던 경로를 고쳤다. raw metadata는 유지하고 worksheet 준비 단계의 **기존 제한된 문맥 추론**으로 넘긴다. 선언·전역 prepass·함수 인자·프로그램 local·미적분 binder·표·표시 단위와 SYSTEM의 기존 판정 경로를 공통 predicate로 연결했다. 직접 raw numeric MathML에서 resolver 없이 `*`/`NONE` 또는 contextual 이름을 실행하면 명시 Unsupported다. 전체 wildcard/type 추론을 복원한 것이 아니다.

기존 raw `*` 프로그램 fixture를 실제 worksheet resolver 경로로 바꾸자 `for k`의 body가 카탈로그 상수 k로 흐르던 별도 공백도 드러났다. 현재 지원하는 named VARIABLE loop binder와 body에만 scope overlay를 적용한다. iterable은 외부 문맥에서 해석하며 caller binding·loop 뒤 lifetime에 새 원본 의미를 주장하지 않는다. symbolic의 별도 namespace 처리 정책은 이번 변경에서 확장하지 않았다.

캐시는 `*`/`NONE`의 실제 조회 의존성을 기록한다. 앞선 변수 선언 추가·삭제, 후속 global 선언과 이름이 겹치는 unit/builtin 변경 때 기존 fragment를 잘못 재사용하지 않는지 일반 builder oracle과 비교한다. 명시 6종의 의미는 유지한다.

## 검사 경계

editor는 순환·원본 None alias·단일 Undo/Redo·문자 선택·suffix·scope·동일 지정 무작업을 검사한다. app은 실제 전체 키 라우팅, 정확한 modifier/release, draft 중 기존 결과 유지·Manual Enter, 자동 리본과 caret·Undo를 검사한다. compat은 원문 왕복·문맥 cache·전역/함수/binder/local/표·단위/SYSTEM·raw Unsupported를 검사한다.

새 native probe는 7개 영역·질의 5개에서 변수 m=7, 명시 단위 m=1 m, sin(0)=0, ORIGIN=0, literal suffix m_dc=4를 확인한다. 자체 저장을 다시 열 때 계산 0회로 표시 5개를 복원한다. native 내보내기에는 `labels="*"` 식별자 4개와 suffix가 남고 contextual true로 바뀌지 않는다. 재수입 뒤 모든 영역은 Math이며 재계산 오류가 없다. 벡터 PDF에서도 해당 이름·결과의 검색 가능성과 이미지 0개를 검사한다.

새 기능을 근거 없이 추가하지 않았으며 기능 단일 기준 [FEATURES](../FEATURES.md)의 EA-016을 미구현에서 부분 구현으로 바꿨다. 553행 중 부분 구현 136·미구현 417이다. 일부 지원 경로와 테스트 개수를 전체 Mathcad 20% 달성으로 환산하지 않는다. 첫 입력 비용과 cold Enter의 남은 병목은 [0090](0090-first-open-input-cost.md)·[0092](0092-committed-input-integration.md)를 따른다. r93의 성능 원시 자료는 별도로 보존한다.

## 최종 r94의 입력 측정

현재 source의 Windows test harness를 별도로 보존하고 r89 before 실행 파일과 5회씩 순서 교대했다. r93 측정과 시점이 다르므로 아래 수치를 r93→r94 변경의 통제된 효과로 비교하지 않는다. [0090](0090-first-open-input-cost.md)의 실제 save→fresh open→빈 frame→focus→첫 이벤트→Enter 방법을 그대로 사용한다.

| 문서 | 첫 Text 전→r94 | Enter 전→r94 | 각 실행 Text+Enter 합의 median 전→r94 |
| --- | ---: | ---: | ---: |
| 4,000 scalar, 저장 cache 있음 | 59.367→2.494 ms | 13.737→60.982 ms | 73.104→63.476 ms |
| 4,000 scalar, 저장 cache 없음 | 60.610→2.324 ms | 14.544→63.037 ms | 74.019→65.577 ms |
| 16×16 행렬 100개, cache 있음 | 47.292→5.637 ms | 5.370→44.489 ms | 52.820→50.325 ms |
| 16×16 행렬 100개, cache 없음 | 53.046→9.320 ms | 9.108→50.566 ms | 62.705→59.717 ms |
| 원본 1015 | 5.032→0.513 ms | 1.312→4.521 ms | 6.195→5.056 ms |
| 원본 1016 | 5.196→0.490 ms | 1.326→4.320 ms | 6.379→4.897 ms |

캐시 있는 4,000영역 Arrow release는 58.980→0.786 ms, 준비 1→0회였다. after 180case의 첫 이벤트 준비는 모두 0회, 비편집의 Enter까지 total은 0회, 실제 수정은 Enter에서만 1회이며 generation 계약 위반·global error는 0개다. peets에서도 별도 36case가 같은 계약을 통과했다.

기록한 open·empty·focus·Text·Enter CPU 단계 합의 median은 캐시 있는 4,000영역 114.178→102.998 ms, 없는 경우 99.548→89.924 ms였다. 행렬은 cache 있을 때 85.684→85.672 ms, 없을 때 **97.749→98.815 ms로 증가**했다. 원본 1015/1016은 52.204→48.854 ms·49.206→47.628 ms다. 이 합에는 font 설정·fixture 생성·caret helper·대기·worker remaining이 없으며 전체 앱 시작 wall time이 아니다. 계산 준비 비용이 Enter로 이동했고 큰 문서 확정 지연은 여전히 남는다. CPU test profile의 동일 PC 표본이며 GPU·OS GUI 체감 성능을 측정하지 않았다.

원시 자료는 ignored `artifacts/rea/label-cycle-open-input-alternating-20261019.{json,log}`, runner는 `measure-label-cycle-open-input-20261019.py`다. after `label-cycle-open-input-after-r94-20261019.exe`의 SHA-256은 `5a133ab4715dcd3697539727fae6caba24881a2c089a988ef2e491c5aa87c255`이며 현재 source에서 생성한 harness와 같다. before identity와 모든 표본·검사 결과는 JSON에 있다. r93의 실행 파일·JSON/log와 0090은 덮어쓰지 않았다.

Enter의 준비 phase도 같은 원시 로그에서 구했다. 아래는 각 phase별 median이며 열들을 더해 전체 median으로 쓰지 않는다.

| 문서 | 서명 | editor 검증 | 직렬화 | worksheet/라벨 | 표시 준비 | 준비 전체 | Enter |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 4,000 scalar, cache 있음 | 7.763 ms | 0.204 ms | 9.865 ms | 26.865 ms | 13.733 ms | 58.357 ms | 60.982 ms |
| 4,000 scalar, cache 없음 | 7.759 ms | 0.173 ms | 10.356 ms | 28.297 ms | 13.604 ms | 60.833 ms | 63.037 ms |
| 원본 1015 | 0.440 ms | 0.082 ms | 0.829 ms | 1.790 ms | 0.704 ms | 3.977 ms | 4.521 ms |

다음 비용 조사는 worksheet의 fragment parse·라벨·cache payload·출력 조립과 표시 준비의 분해다. 표시 worker 분리는 source/editor revision·generation·draft 중 F5·print snapshot의 계약까지 필요하므로 이 표만으로 작은 수정이라고 판단하지 않는다.

## 동일 소스의 양 OS 검증

Windows/peets 각각 **workspace 764개 + backend 2개 = 766개 통과·실패 0개·29개 ignored**다. app 337개, compat 261개, document 70개, editor 83개, core 13개다. fmt·workspace all-target Clippy·별도 backend library/test Clippy(`-D warnings`)·workspace 빌드를 통과했고 Windows release를 다시 빌드했다. 원본 참조 38개는 saved/input 양쪽 각각 38/38·차이 0이다.

source archive는 ignored `artifacts/rea/mathcraft-label-cycle-20261019-r94.tar`, SHA-256 `dee4f7f880b43e1d76b6e9ee4da1fa18a397e695114331017c91b6ddfb10f284`, 7,526,400 bytes다. 제품 Rust 106개·vendor Rust 5개를 포함한 manifest 129파일이 archive·Windows·peets 실행 후 소스와 같다. peets fresh root는 `/home/peets/Projects/mathcraft-label-cycle-20261019-r94`다. Windows `target/release/mathcraft.exe`는 **22,016,000 bytes**, SHA-256 `3c7b28e0b4394d2e32263efc4be4d5aefc61517ad156b23796af42a4a1f30123`다. 일반 앱 main은 실행하지 않았다.

양 OS 각각 native probe 10개·PDF probe 8개를 수행했다. MCDX **20개·XML 80파트**의 설치 XSD 오류는 0개다. 원본 표 계산·색상·64개 저장 표시 및 XamlPackage 원문 유지 검사를 통과했다. PDF **16개 모두 이미지 0개**, 두 reader의 검색 텍스트·원본 표 숫자/색상/기준점 검사를 통과했다. 새 라벨 PDF는 OS별 1쪽·복원 표시 5개·벡터 drawing 129개다. 원본 전체 배치·모든 글꼴·실제 물리 프린터의 동등성은 미검증이다.

검증 root는 ignored `artifacts/rea/label-cycle-integration-20261019`다. `source-validation.json`에 archive·소스·release·검사·성능 identity를, `schema-validation.json`과 `pdf-verified/validation.json`에 native/PDF 결과를 기록했다. `peets/verification.json`의 산출물 42개·표준 원격 로그 5개·별도 실제 열기 로그는 수집 후 size/SHA를 다시 확인했다. 표준 로그는 `label-cycle-{windows-fmt,windows-clippy,windows-tests,backend-windows,backend-windows-clippy,windows-build,windows-reference,windows-probes,peets-check,peets-backend-clippy,peets-reference,peets-probes,peets-pdfs}-20261019.log`, 별도 raw profile은 `label-cycle-open-input-{windows,peets}-20261019.log`다.
