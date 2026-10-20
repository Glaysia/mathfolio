# 수식 확정 준비의 문맥 라벨 재처리 비용

2026-10-19. [0062](0062-input-preparation-second-pass.md) 이후에도 수식 하나를 확정할 때 변경되지 않은 모든 영역의 MathML 문맥 라벨과 표시 tree를 다시 만들고 있었다. 원본의 draft/Enter 경계와 전역 정의·문맥 라벨 계약은 유지하면서 이 중복 작업을 줄였다. GUI, 스크린샷, 원본 런타임, COM, CLR은 사용하지 않았다. 이번 최적화는 Rust 소스와 headless 차등 테스트를 근거로 하며, 원본의 새로운 계산 규칙을 추측해 추가한 변경이 아니다.

## 변경한 경로

`WorksheetXmlCache`는 계산 결과가 아니라 **라벨을 결정한 MathML fragment**를 보존한다. 재사용하려면 region ID와 원본 XML 전체 바이트가 같고, 해당 fragment의 라벨 처리에서 실제로 조회한 worksheet 식별자의 값이 모두 같아야 한다. 조회 결과의 부재도 `None`으로 보존한다. 따라서 원래 내장 함수나 단위였던 이름 앞에 새 정의를 추가하면 cache를 버리고 다시 라벨 처리한다. 식별자의 literal subscript도 lookup identity에 포함한다.

캐시 적중 시에도 그 fragment가 만든 정의를 현재 문맥에 다시 반영한다. 문서 전체의 globalDefine 선처리, 기하학적 stable order, 중복 ID·좌표·XML 크기 검증은 기존 `worksheet_xml` builder가 수행한다. 명시 라벨이나 contextual metadata를 제거하는 정규화, 값의 유효성이나 계산 성공 여부를 기억하는 cache는 넣지 않았다. SpecTable은 기존 labeller를 매번 사용하며 표가 만든 정의 효과를 뒤따르는 수식의 lookup 검사에 반영한다.

초기 구현은 모든 식별자를 문맥 의존 항목으로 기록했다. 재검토 결과 명시된 noncontextual 라벨, 정의 좌변, 구조상 고정된 라벨과 bound local은 worksheet binding을 조회하지 않는다. 이 경로는 의존 항목을 만들지 않도록 교정했다. 내장 함수 판정에서 사용한 binding의 부재와 일반 추론 경로에서 조회한 binding은 계속 기록한다. 출력이 바뀌지 않는 global/context 변경 때문에 식을 다시 처리하거나, 4,000개 단순 선언마다 불필요한 dependency map을 할당하던 비용을 줄인다.

앱의 표시용 tree cache는 완성된 `math` 노드의 전체 XML 바이트가 같은 경우에만 기존 tree를 쓴다. 원본 fragment가 같더라도 전역 정의나 앞선 정의 때문에 생성 라벨이 달라지면 표시 tree를 다시 만든다. 제거·비활성화·실패로 표시 tree를 해석할 수 없어진 영역은 표시 cache에서 정리한다. 배치 전용 cache는 [0062](0062-input-preparation-second-pass.md)의 설정·archive 바이트·문맥 순서 검사를 계속 사용한다.

## 측정 범위와 전후 값

Windows Cargo test profile에서 자체 코드는 opt-level 1, dependency는 opt-level 2다. CPU headless 이벤트와 자체 worker를 사용했으며 실제 데스크톱 창의 프레임률이나 GPU 품질을 측정하지 않았다. 이전 측정은 10월 16일 cache 도입 전 로그이고, 재검증 측정은 10월 19일이다. 시간대가 다른 단일 실행이므로 모든 차이를 통제된 성능 보장으로 해석하지 않는다.

| 동일 workload | cache 도입 전 | 재검증 후 |
|---|---:|---:|
| 4,000 scalar, 숫자 하나 변경 후 계산 입력 준비 | 41.097 ms | 20.717 ms |
| 위 준비 중 worksheet 생성·문맥 라벨 단계 | 22.069 ms | 4.548 ms |
| 위 준비 중 표시 tree 단계 | 12.465 ms | 8.609 ms |
| 4,000 scalar, 실제 draft key frame | 0.847 ms | 1.054 ms |
| 4,000 scalar, 확정·worker 제출까지 | 46.349 ms | 27.600 ms |
| 4,000 scalar, 실제 cold retain | 50.945 ms | 57.803 ms |
| 16×16 행렬 100개, 확정·worker 제출까지 | 28.545 ms | 15.947 ms |
| 실제 1015 문서 107영역, 확정·worker 제출까지 | 2.954 ms | 1.650 ms |

27.600 ms는 Enter에 대응하는 확정 함수의 CPU 처리·worker 제출까지의 시간이다. 별도의 `worker_remaining_us=27185`를 포함한 전체 계산 완료 시간이 아니다. key frame에서는 초안만 변경하고 계산된 영역이 없음을 검사한다. 실제 1015 문서는 확정 후 오류 없이 80개 계산 행을 얻는다.

4,000 scalar cold 준비는 이전 52.811 ms, 재검증 56.153 ms였다. **첫 준비는 더 빨라졌다고 주장하지 않는다.** 정확한 XML과 문맥 의존 항목·표시 소스를 보존하는 비용이 추가된다. 같은 실행 파일 안에서 4,000개 선언의 라벨 단계만 11회 반복하고 uncached/cached의 선행 순서를 번갈아 실행한 median은 다음과 같다.

| 순수 worksheet 라벨 생성 | uncached | cached |
|---|---:|---:|
| cold | 19.315 ms | 22.515 ms |
| 숫자 하나 변경 후 | 19.218 ms | 4.080 ms |

cold 라벨 단계의 추가 비용은 이 측정에서 3.200 ms다. 첫 입력 준비, 전체 worksheet parse, 내용 서명, 이벤트·확정 경로와 worker 계산은 여전히 남는다. 4,000개 영역에서 확정 제출 27.600 ms도 한 프레임보다 길 수 있으므로 버벅임이 모두 해결됐다고 판단하지 않는다.

## 메모리와 실패 경계

fragment cache에는 지역별 한 개의 cacheable source/labelled fragment와 조회 문맥을 보존한다. 산정한 보존 payload 한도는 32 MiB이며 초과 시 cache를 비우고 동일 builder로 계속 처리한다. 문서 이력을 무한히 보관하지 않는다. 4,000개 단순 선언에서 계측한 값은 원본 fragment 261,780 bytes에 대해 cache payload **1,338,450 bytes**였다.

이 숫자와 한도는 전체 RSS나 allocator 한도가 아니다. outer hash table의 bucket/control byte, allocator overhead, 임시 XML 문서·완성 worksheet, 앱의 기존 직렬화·geometry·표시 cache와 native archive 등은 포함하지 않는다. 전체 프로세스 메모리를 32 MiB로 제한했다는 의미도 아니다. 계산 결과와 engine scope는 이 cache에 보존하지 않는다.

실패한 빌드가 직전 계산값을 성공으로 복원하거나 poison을 지우지 않도록 cached 출력과 uncached 출력을 그대로 비교한다. 일반 정의 실패, 잘못된 전역 정의, 중복 전역 정의, 잘못된 XML·contextual metadata를 거부하는 결과도 비교한다. GUI·원본 실행을 대신한 테스트라는 범위를 유지한다.

## 검증과 재현

cache 단위 테스트는 9개 통과, profile 2개는 기본 실행에서 ignored다. 앱의 `worksheet_ui::edit_commit` 묶음은 10개 통과, profile 4개 ignored였다. metadata·전체 이벤트·준비 단계 profile은 따로 직접 실행해 통과했다. 별도 차등 audit는 builtin/function shadow, contextual 선언 라벨 변경, suffix와 bound local, 잘못된/중복/삭제된 global, 비활성 표, 실패 후 복구와 32 MiB 초과 eviction을 확인한다. 전체 Windows·peets 검증은 같은 소스 freeze 이후 [CONFORMANCE](../CONFORMANCE.md)에 기록한다.

```powershell
cargo test -p mathcraft-compat --lib worksheet_cache --locked --offline
cargo test -p mathcraft-app --bin mathcraft worksheet_ui::edit_commit --locked --offline
cargo test -p mathcraft-compat --lib profile_cold_and_edited_worksheet_labelling --locked --offline -- --ignored --nocapture --test-threads=1
cargo test -p mathcraft-compat --lib profile_four_thousand_fragment_cache_memory --locked --offline -- --ignored --nocapture
cargo test -p mathcraft-app --bin mathcraft preparation_phase_profile --locked --offline -- --ignored --nocapture --test-threads=1
cargo test -p mathcraft-app --bin mathcraft benchmark_draft_commit_and_worker_latency --locked --offline -- --ignored --nocapture --test-threads=1
```

근거 로그는 `artifacts/rea/commit-preparation-before-{phases,events}-20261016.log`, `commit-preparation-after-{phases,events}-20261016.log`, `commit-preparation-recovery-{cache-tests,focused,commit-tests,metadata-profile,phases,events}-20261019.log`다. 첫 cache 구현의 cold regression과 이번 측정도 함께 보존했다. 전체 완성률 20%를 이 성능 수치나 테스트 수에서 도출하지 않는다.
