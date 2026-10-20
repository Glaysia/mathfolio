# 생성된 계산 입력의 전체 DOM 재해석 비용

2026-10-19. [0079](0079-commit-preparation-cost.md)에서 문맥 라벨과 표시 tree를 보존했지만, 숫자 하나가 바뀔 때도 완성된 worksheet 전체를 다시 XML DOM으로 읽었다. 4,000영역에서는 이 표시 준비 단계가 8.817 ms였다. 이번 변경은 기존 Rust builder의 출력 위치를 직접 전달해 중복 해석을 줄인다. 원본의 새 기능이나 계산 의미를 추가한 작업이 아니며 GUI·스크린샷·원본 런타임을 사용하지 않았다.

## 구현과 보존 경계

`WorksheetXmlCache::build_prepared`는 기존 `build`와 같은 검증·라벨 처리·전체 XML 바이트에 각 top-level region의 ID, left/top 속성 값 범위와 직접 자식 math의 범위를 더한다. 범위는 출력 문자열을 만드는 순간 기록한다. 식별자 UTF-8 길이, literal subscript, 전역 정의와 앞선 선언의 라벨 문맥을 범위 산정에서 추측하지 않는다. 기존 공개 `worksheet_xml`과 `build`의 문자열 API도 유지한다.

앱은 이 범위에서 기하 위치 변경용 template을 잡는다. 표시 cache가 이미 채워졌으면 각 math의 **완성된 XML 전체 바이트**를 비교하고 바뀐 fragment만 `tree_from_mathml`로 해석한다. 고정 WS/ML namespace는 bridge의 기존 wrapper가 제공하고, fragment 내부의 추가 namespace 선언은 그대로 보존한다. 동일 길이의 다른 문자열, source pointer identity, region ID만으로 표시 tree를 재사용하지 않는다.

첫 표시 cache를 채울 때에는 전체 DOM을 한 번 사용한다. 초기 시제품에서 4,000개 작은 fragment를 각각 parse하면 cold 준비가 61.682→70.482 ms로 느려져 이 경로를 채택하지 않았다. 기존 전체 DOM에서 모든 노드를 읽는 방식이 cold에는 유리했다. Solveblock은 새 XML을 삽입하면서 nested region·namespace scope가 생길 수 있으므로 삽입 후 전체 DOM 경로를 유지한다. SpecTable은 기존 labeller를 사용하며 builder가 직접 math를 감싸지 않은 table region은 표시 tree 범위를 만들지 않는다.

geometry 재사용은 기존의 문서 width/height/grid 비트, native archive 전체 바이트, 모든 statement ID/XML 전체 바이트, stable geometric order 비교를 계속 통과해야 한다. 이동 이후 값 길이가 바뀌어도 원래 template의 정확한 좌표 범위를 한 번의 선형 복사로 다시 쓴다. 전역 prepass·duplicate ID·비정상 좌표·XML 예산·비활성화·실패 poison·Undo/재정렬에 대한 기존 검증을 축소하지 않았다.

## 같은 PC에서의 전후 측정

Windows Cargo test profile, headless CPU 측정이다. 자체 코드 opt-level 1·dependency opt-level 2이며, 실제 GPU 프레임률이나 데스크톱 창의 체감을 측정한 값은 아니다. 변경 직전과 변경 후 같은 fixture·동일 probe를 실행한 단일 표본이다. 다른 작업과 CPU 상태를 완전히 통제한 median이나 성능 보장으로 확대하지 않는다.

| 경로 | 변경 직전 | 변경 후 |
|---|---:|---:|
| 4,000 scalar, 숫자 하나 변경 후 준비 | 22.131 ms | 15.312 ms |
| 위 준비의 표시 단계 | 8.817 ms | 2.384 ms |
| 4,000 scalar, cold 준비 | 61.682 ms | 63.002 ms |
| 4,000 scalar, 실제 draft key frame | 1.069 ms | 1.082 ms |
| 4,000 scalar, 확정·worker 제출 | 26.046 ms | 21.915 ms |
| 4,000 scalar, cold retain | 60.808 ms | 62.308 ms |
| 16×16 행렬 100개, 숫자 변경 후 준비 | 10.458 ms | 6.691 ms |
| 16×16 행렬 100개, 확정·worker 제출 | 16.484 ms | 11.118 ms |
| 실제 1015 107영역, 확정·worker 제출 | 1.545 ms | 1.414 ms |

첫 준비는 빨라졌다고 주장하지 않는다. 완성된 출력의 metadata Vec를 추가 생성하는 비용이 있고, 그중 좌표 범위는 이전에도 geometry cache에 보존하던 항목이다. fragment 범위 metadata는 준비 중 임시 값이며 문서 이력이나 계산 결과를 보존하지 않는다. 이 측정은 RSS 측정이 아니다.

4,000영역 확정 후 worker 잔여 시간은 변경 전 27.483 ms·변경 후 29.305 ms였다. 위 확정·제출 시간은 전체 계산 완료 시간이 아니다. 내용 signature와 기존 직렬화·라벨 생성·worker 계산은 여전히 남으며, 확정 준비가 아직 한 프레임을 넘을 수 있다. 전체 성능 문제 해결이나 제품 완성률 증가를 검사 수로 환산하지 않는다.

## 차등 검증과 재현

compat의 새 검사 두 개는 기존 `worksheet_xml`과 **정확한 문자열**을 비교하고, 독립적으로 전체 DOM을 만들어 emitted range와 실제 속성/node 범위를 비교한다. region ID 0/u64::MAX, -0·큰 지수 좌표, 한글·µ·namespace 있는 literal subscript, 앞/뒤 정의와 global 전환, SpecTable, 빈 문서, NaN 좌표·중복 ID·잘못된 XML을 포함한다. 각 math의 fragment bridge 결과도 동일 DOM node bridge와 비교한다.

앱의 `input_latency` 검사는 cache를 비운 결과와 기존 전체 DOM 기반 표시 oracle을 함께 비교한다. 새 검사는 UTF-8 식별자 수정·자리수가 다른 값·큰 좌표 이동·비활성화·실패를 거친 뒤에도 표시 tree가 같은지 확인한다. test-only 카운터로 cold 한 번 이후 해당 수정에서는 전체 DOM을 다시 만들지 않았음을 검사한다. 기존 global 문맥·ORIGIN 순서·동일 길이 소스 변경·좌표 길이 변경·문서 설정·동일 길이 archive 변경·실패/제거 정리·Solveblock fallback 검사를 유지한다.

명령은 `cargo test -p mathcraft-compat generated_ -- --nocapture`, `cargo test -p mathcraft-app edit_commit -- --nocapture`다. 성능은 앱 test harness의 `preparation_phase_profile --ignored --nocapture`와 `benchmark_draft_commit_and_worker_latency --ignored --nocapture`로 재현한다. test harness는 자체 CPU 테스트이며 Mathcraft main 앱을 실행하지 않는다.

원시 로그는 ignored `artifacts/rea/prepared-source-{before,after,events-before,events-after,metadata-tests,tests,final-tests}-20261019.log`에 있다. 이 문서는 해당 경로의 독립 소스·차등 검사 근거다. 최종 Windows/peets 전체 소스 검증과 MCDX/PDF 회귀 결과는 통합 기록에 별도로 남긴다.
