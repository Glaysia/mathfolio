# 0187 — 런타임 String 결과 공유 비용

2026-10-19. 승인된 기준점은 r202이다. 이 문서의 비교는 r202를 별도 디렉터리에 추출한 원본과 `StringValue(Arc<str>)` 후보 사이의 Windows 헤드리스 compat 측정이다. 후보는 이후 main에 통합되었고 workspace all-target check가 통과했지만, 작성 시점의 main 전체 테스트·양 OS·native/own/worker 통합 검증은 **진행 중**이다. 아래 결과를 main의 최종 검증 완료나 앱 전체 속도로 해석하지 않는다.

## 바뀐 소유권과 유지한 계약

런타임 `Value::String.value`만 불변 `StringValue`로 바꿨다. `StringValue`는 `Arc<str>`를 소유하며 clone은 같은 텍스트를 공유한다. 독립적으로 계산한 같은 문자열을 intern하거나 계산을 재사용하지 않는다. editor/PortableTree/Expression의 소스 문자열은 기존 `String` 그대로이다.

`as_string() -> Result<&str>`, 내용 기반 equality, Debug/Display, `{"kind":"string","value":...}` 필드·순서·escaping과 기존 유효성 검사 위치를 유지한다. custom serde는 직렬화 시 `str`를 쓰고 역직렬화 시 기존 `String::deserialize` 후 공유 carrier로 옮긴다. serde의 `rc` feature는 추가하지 않았다. 공개 Rust 필드를 직접 구성하던 코드는 `.into()`가 필요하며, 앱 테스트의 소유 `String` 반환 helper는 `.to_string()`으로 적응했다.

기능 후보의 제품 변경은 compat `lib.rs`, `strings.rs`, `string_tests.rs`, 신규 `string_value.rs` 네 경로이다. 측정용 `string_sharing_profile.rs`, `string_sharing_alloc.rs`, Cargo feature와 등록은 **scratch 전용**이며 제품에 추가하지 않았다. main의 동시 진법 작업은 이 비교 소스에 포함되지 않는다.

`StringLiteral` 평가에서는 검증한 소스 `&str`를 Arc로 한 번 복사한다. 반대로 concat의 완성된 `String`과 역직렬화한 `String`을 Arc로 옮길 때는 추가 할당·복사가 생긴다. 마지막 소유자가 해제될 때까지 텍스트가 살아 있으며 전역 보존 캐시는 없다. 결과 전체의 새 메모리 예산이나 행렬 공유 정책은 추가하지 않았다.

## 고정 소스와 실행체

원본 archive는 `mathcraft-string-values-20261019-r202.tar`, SHA-256 `6090b27fa0163abd511d973cb91f693e8821448cab5f4ebac6fc0540b67a5795`이다. 추출 시 [원본 manifest](../../artifacts/rea/string-sharing-next-20261019/baseline-manifest.json)의 501개 파일을 대조했다. 이후 동일한 측정 모듈을 양쪽에 넣고 각각 별도 `target-baseline`, `target-candidate`로 빌드했다.

| 측정 소스 manifest | SHA-256 |
|---|---|
| [원본+동일 계측](../../artifacts/rea/string-sharing-next-20261019/measurement-baseline-source-manifest.json) | `34f1f61799e108498078043240f0d5e7ac4d34340493fc769231ce94f1c82135` |
| [공유 후보+동일 계측](../../artifacts/rea/string-sharing-next-20261019/measurement-candidate-source-manifest.json) | `d41ce3e0888d5365dc75ad96c70e0cbad10a076bd89350169736b23ff8c865cd` |

| 고정 실행체 | 관찰자 | SHA-256 |
|---|---|---|
| [원본 off](../../artifacts/rea/string-sharing-next-20261019/string-sharing-isolated-baseline-off.exe) | 없음 | `e67caf33d658757ef5be55e273c7efd9e5aa0495d8888c4541966ff3584297fc` |
| [후보 off](../../artifacts/rea/string-sharing-next-20261019/string-sharing-isolated-candidate-off.exe) | 없음 | `6e4340e3c362374fdae05baa80758e82e804d3d76050787e7f71763ba7160860` |
| [원본 on](../../artifacts/rea/string-sharing-next-20261019/string-sharing-isolated-baseline-on.exe) | 할당 계수 | `d42e243484da601fd2ba0f9af09b7b31c017f6868b56db91636426f37623b7b7` |
| [후보 on](../../artifacts/rea/string-sharing-next-20261019/string-sharing-isolated-candidate-on.exe) | 할당 계수 | `20acd128e0856d69e58276e16fa9283815158c8162b6933e47446afa4d84a0fb` |

전체 소스·실행체·12개 raw 결과의 해시는 [measurement-identities.json](../../artifacts/rea/string-sharing-next-20261019/measurement-identities.json)에 있다. 빌드 직후 `--list`에서 후보 전용 byte oracle 세 개의 존재와 원본에서의 부재를 검사했다.

초기에는 별도 소스 디렉터리가 같은 scratch target을 사용했고 Cargo의 `fresh=true` 재사용으로 원본/후보 실행체가 같은 SHA가 되었다. 이 네 실행체는 **측정 전에 기각**했다. [기각 기록](../../artifacts/rea/string-sharing-next-20261019/shared-target-build-rejected.json)과 기존 빌드 로그를 보존했으며 아래 수치는 전부 `isolated` 실행체에서만 나왔다. 이는 측정 결과를 제외한 빌드 실패이며, 오염된 timing을 정상 표본에 섞지 않았다.

## 측정 방법과 범위

root/A의 Cargo·Clippy·프로파일러 종료 및 quiet 확인 후 `2026-10-19T13:25:51.712141Z`부터 `13:26:05.576459Z`까지 실행했다. C의 작업도 종료된 상태였다. [runner](../../artifacts/rea/string-sharing-next-20261019/run-measurement.py)는 원본/후보를 B/C, C/B, B/C, C/B, B/C로 5회 교대했다. 이 10개 프로세스에는 global allocator wrapper가 없다. 이어 별도 원본/후보 각 1개 프로세스에서 할당 관찰자를 켰다. 할당 관찰자의 시간은 CPU 비교에 사용하지 않았다. [UTC·종료 코드](../../artifacts/rea/string-sharing-next-20261019/measurement-execution-log.json)와 [경계](../../artifacts/rea/string-sharing-next-20261019/measurement-boundary.json)를 보존했다.

Windows compat test profile로 workspace opt-level=1, 의존성 opt-level=2이다. release/LTO 앱, UI frame, GPU, Linux 성능을 측정하지 않았다. GUI·스크린샷·설치된 Mathcad/CLR 실행도 하지 않았다.

fixture는 빈 문자열, 32B, 8KiB, 256KiB와 0/1/8/32개 조회 및 숫자 대조군이다. 가장 큰 입력은 16KiB 리터럴 16개를 concat해 256KiB 값을 만든다. `s:=concat(...)`와 `s=` 조회의 XML은 공유 후보의 런타임 Value에서 역생성하지 않고 평문으로 독립 작성한다. 12개 프로세스의 입력 XML bytes가 전부 같음을 확인했다. 수천 개 1MiB 결과를 만드는 위험한 부하를 사용하지 않았다.

계산기 source update, 최초 실제 계산, 이미 계산된 값의 zero-execution snapshot, 동일 소스 update, query 준비, parse+query 평가, 준비된 query 평가, clone, concat, cold deserialize, serialize를 분리했다. setup·값/wire 검증·소스 작성·영구 계산기 해제는 action timer 밖이다. action 후 검증을 거쳐 반환값 drop을 별도로 측정한다. action+drop은 각 표본의 두 시간 합이며 연속 wall time이 아니다. 검증이 뒤 표본의 캐시·할당자 상태에 영향을 줄 수 있다. 서로 다른 phase median을 더해 전체 시간이라고 주장하지 않는다.

## 계산·snapshot 결과

단위 µs. 각 값은 5개 프로세스별 median의 median이며 원본 → 후보이다. [전체 원시 요약](../../artifacts/rea/string-sharing-next-20261019/measurement-summary.json) 및 [상세 표](../../artifacts/rea/string-sharing-next-20261019/MEASUREMENTS.md)에 action+drop도 보존했다.

| fixture | 최초 계산 | 실행 0회 snapshot | 동일 소스 update | cold source update |
|---|---:|---:|---:|---:|
| empty / 32조회 | 25.2 → 25.4 | 4.3 → 4.3 | 3.9 → 4.0 | 140.2 → 128.8 |
| 32B / 32조회 | 21.5 → 19.2 | 6.1 → 4.9 | 5.5 → 4.3 | 119.9 → 114.7 |
| 8KiB / 32조회 | 139.8 → 104.0 | 10.9 → 4.6 | 10.1 → 4.3 | 162.7 → 157.6 |
| 256KiB / 0조회 | 2771.8 → 2813.7 | 0.5 → 0.6 | 8.5 → 8.4 | 1406.2 → 1398.9 |
| 256KiB / 1조회 | 2941.5 → 2963.9 | 84.6 → 1.4 | 98.6 → 11.5 | 1471.2 → 1377.5 |
| 256KiB / 8조회 | 4068.8 → 2887.3 | 705.0 → 3.5 | 694.5 → 15.4 | 1497.7 → 1431.7 |
| 256KiB / 32조회 | 8489.8 → 2843.7 | 3149.2 → 15.0 | 3052.9 → 34.3 | 1563.4 → 1538.7 |
| numeric / 32조회 | 17.2 → 17.2 | 4.2 → 4.2 | 3.6 → 3.7 | 116.9 → 85.3 |

256KiB/32조회에서 action+drop은 최초 계산 9026.5 → 2875.3µs, snapshot 3707.1 → 46.7µs, 동일 소스 update 3748.3 → 54.8µs였다. 최초 계산은 양쪽 모두 정의+조회 33개를 실행했고 snapshot은 양쪽 모두 실행 0회였다. 계산 횟수 축소 없이 binding·결과 row·context·반환값에 따른 반복 payload 복사를 줄인 결과이다. 숫자 source update에도 차이가 있으므로 source parse의 감소까지 공유 효과로 귀속하지 않는다.

## constructor·작은 입력의 불리한 결과

단위 µs, 같은 집계 방식이다. 0.0은 0ns 비용을 증명하지 않으며 일부 단일 clone은 100ns 시계 해상도 이하이다.

| fixture | 준비 query 평가 | query 준비 | Value clone | concat | cold deserialize | serialize |
|---|---:|---:|---:|---:|---:|---:|
| empty / 32조회 | 0.2 → 0.1 | 6.7 → 7.2 | 0.0 → 0.0 | 0.1 → 0.1 | 0.3 → 0.3 | 0.1 → 0.1 |
| 32B / 32조회 | 0.2 → 0.2 | 6.1 → 6.2 | 0.0 → 0.0 | 0.3 → 0.4 | 0.3 → 0.4 | 0.1 → 0.1 |
| 8KiB / 32조회 | 0.3 → 0.2 | 5.9 → 6.0 | 0.2 → 0.0 | 59.1 → 59.6 | 1.9 → 2.1 | 4.1 → 4.1 |
| 256KiB / 0조회 | 72.8 → 0.4 | 6.2 → 6.3 | 67.9 → 0.0 | 1987.0 → 2048.4 | 128.6 → 163.6 | 122.6 → 250.0 |
| 256KiB / 1조회 | 73.1 → 0.4 | 6.5 → 6.6 | 126.3 → 0.1 | 1976.0 → 1949.2 | 114.5 → 65.9 | 120.3 → 119.2 |
| 256KiB / 8조회 | 73.5 → 0.3 | 6.6 → 6.7 | 64.0 → 0.0 | 1916.7 → 1997.4 | 50.9 → 124.8 | 119.4 → 132.3 |
| 256KiB / 32조회 | 9.1 → 0.3 | 6.3 → 6.6 | 13.0 → 0.0 | 1910.2 → 2004.5 | 48.5 → 125.6 | 119.0 → 120.4 |
| numeric / 32조회 | 0.2 → 0.2 | 6.0 → 6.4 | — | — | — | — |

공유할 후속 조회가 없는 큰 최초 계산은 2.7718 → 2.8137ms로 악화했다. 1조회 계산도 action은 증가했다. concat·역직렬화는 추가 Arc 복사 비용을 가지므로 항상 빨라지는 최적화가 아니다. 특히 0조회 serialize 122.6 → 250.0µs 악화의 원인은 확정하지 못했다. query 수에 따라 constructor/serialize 분포가 달라지는 원시 자료도 숨기지 않았다. 짧은 clone이나 한 fixture의 역직렬화 개선을 보편 비율로 주장하지 않는다.

## 할당 계수와 보존 비용

[별도 allocator 요약](../../artifacts/rea/string-sharing-next-20261019/allocation-summary.json)은 성공한 Rust alloc 요청, realloc 요청과 논리적 requested live bytes를 센다. RSS, OS heap 크기, allocator 내부 fragmentation, System realloc 내부의 물리적 old/new 동시 보존은 측정하지 않는다. 아래 peak는 해당 action 시작점 대비 증가이며 시간 표본과 같은 계측 실행이 아니다.

| 작업 | alloc calls | alloc requested bytes | action incremental logical peak bytes |
|---|---:|---:|---:|
| 256KiB/32조회 최초 계산 | 263 → 200 | 17,324,109 → 809,309 | 17,062,193 → 790,540 |
| 같은 snapshot | 103 → 71 | 8,409,305 → 20,697 | 8,409,305 → 20,697 |
| 같은 동일 source update | 99 → 67 | 8,409,050 → 20,442 | 8,409,050 → 20,442 |
| 같은 prepared query | 1 → 0 | 262,144 → 0 | 262,144 → 0 |
| 같은 Value clone | 1 → 0 | 262,144 → 0 | 262,144 → 0 |
| 256KiB concat | 1 → 2 | 262,144 → 524,304 | 262,144 → 524,304 |
| 256KiB deserialize | 2 → 3 | 262,400 → 524,560 | 262,400 → 524,560 |
| 256KiB/0조회 최초 계산 | 39 → 40 | 526,901 → 789,317 | 528,124 → 790,540 |
| numeric/32조회 최초 계산 | 181 → 181 | 21,677 → 21,677 | 22,833 → 22,833 |

최초 32조회 계산의 realloc 요청은 양쪽 모두 6회/8448B이다. 숫자 최초 계산은 양쪽 모두 4회/2880B이다. 빈 concat은 원본 0회에서 후보 1회/16B가 되었고, 빈 역직렬화 요청도 256 → 272B가 된다. 원래 빈 문자열 clone은 할당하지 않았으므로 공유로 제거할 payload 복사가 없었다.

`size_of::<Value>()`는 **양쪽 모두 232B**이다. 다른 enum variant가 전체 크기를 결정하므로 String 필드 축소를 Value 전체 크기 감소로 주장하지 않는다. Arc의 refcount 연산과 헤더가 추가되고 소유자가 오래 살면 payload도 오래 산다. 공유 가능한 큰 조회 행에서는 중복 텍스트가 줄지만, 서로 다른 결과나 직렬화 output의 총량 제한을 해결하지 않는다.

## byte oracle와 검증 상태

기능 후보는 [독립 원본 wire enum 대비 검사](../../artifacts/rea/string-sharing-next-20261019/source/crates/mathfolio-compat/src/string_value.rs)에서 빈 문자열, 숫자처럼 보이는 문자열, escaping/CR/LF/tab/한글/보충 Unicode, generic wire의 XML-invalid 문자, 최대 1MiB를 비교했다. 전체 JSON bytes, `numeric_json`, Debug 및 decode→encode bytes가 같다. 누락/null/잘못된 타입/중복 value의 serde 오류 문자열도 기존 enum과 같으며 generic Value의 기존 unknown-field 허용을 새롭게 확대하거나 축소하지 않았다. 엄격 inert cache admission은 별도 caller 그대로 필요하다. clone의 실제 Arc 공유, 독립 equal 값의 내용 equality, 마지막 소유자 drop과 Send+Sync도 검사했다.

[기능 검사 기록](../../artifacts/rea/string-sharing-next-20261019/validation.json)은 compat 전체 371 pass / 0 fail / 13 ignored 및 all-target Clippy `-D warnings` 통과를 담는다. 이는 계측 추가 전 후보의 검사이며 그때의 별도 manifest·실행체를 보존한다. 12개 비용 프로세스에서는 값·독립 old wire·실행 횟수·동일 source bytes를 다시 검사했다. 이 문서는 기존 native 결과 계약을 새롭게 REA했다고 주장하지 않으며, 원본 프로그램의 공유 방식도 추정하지 않는다.

제품 통합 판단은 큰 문자열의 반복 clone을 실제로 제거한다는 CPU·할당 증거와 constructor 악화를 함께 근거로 한다. r202는 승인된 과거 기준점으로 유지되며, 이후 main 검증이 완료되기 전까지 이 문서의 제한된 compat 실험을 새 완료 checkpoint로 사용하지 않는다.

## 후속 main 통합 검증

[0189의 r203](0189-radix-and-string-sharing-integration.md)에서 같은 고정 소스의 Windows/peets 전체·기존 String native/own/cold/worker 경로와 새 진법 흐름을 통과했다. 위 비용 측정은 독립 scratch 표본이며, 통합 성공을 새로운 앱 전체 성능 측정으로 해석하지 않는다.
