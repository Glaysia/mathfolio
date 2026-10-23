# Windows 실제 창의 소프트웨어 렌더링 병목

2026-10-23. 사용자가 GUI 사용 금지를 해제했고, 최신 peets release에서는 버벅임이 없다고 확인했다. Windows의 실제 사용자 세션에서 자체 앱의 native 창과 surface를 생성해 같은1015 계산서를 반복 스크롤했다. 원본 Mathcad는 실행하지 않았다.

## 확인한 경로

Windows 실행 환경은 QEMU Q35 가상머신이다. 장치 조회에는 Microsoft Basic Display Adapter와 세션2의 Microsoft Remote Display Adapter가 있다. **실제 앱의 CreationContext에서** 선택된 어댑터는 DX12 `Microsoft Basic Render Driver`, DeviceType `Cpu`, driver `10.0.19041.546`이었다. 이제 headless 선택 결과로 실제 창의 어댑터를 추정하는 상태는 아니다.

현재 Windows 앱은 WGPU를, Linux 앱은 Glow를 사용한다. Windows의 하드웨어 가속이 없는 환경도 WGPU의 DX12 CPU fallback으로 그대로 실행된다. 이 환경에서는 UI 조판이 빨라도 렌더러를 거치는 전체 프레임이 느리다. 특정 위젯·입력 큐 최적화만으로 전체 스크롤을 해결한다는 접근은 이 병목을 다루지 못한다.

## 같은 실행체의 대조 실험

최적화된 `native-profile` 진단 실행체 SHA256:

`182dcf861a99b76fa741a9d048b5fadda1380b08036a92aa950079f4d77fb21b`

입력1015_LCC_S.mcdx SHA256:

`217994a631de58e096a3cfdb8a8251aca91501e62151f869950b99f738fc18b5`

각 실행은112 native 입력 프레임이다.16 warm-up,40 아래 이동,40 위 이동,16 settle로 구성한다. native 시간·DPI를 유지하고 실제 App의 입력 hook과 ScrollArea로 합성 휠을 전달한다. 표의 시간은17–96번 프레임의 raw-input 시작 간격이며, OS 입력 수신 지연 또는 실제 모니터 표시 간격은 아니다. 모든 실행에서 문서·저장 결과·계산 제출 세대·Undo/Redo·dirty 불변을 확인했다.

| 조건 | 프레임 시작 간격 중앙값 | p95 | UI 코드 중앙값 |
| --- | ---: | ---: | ---: |
| 전체 App, 기본 AutoVsync/latency1 | 218.963ms | 220.064ms | 0.995ms |
| 빈 화면 대조군 | 62.799ms | 93.727ms | 0.0009ms |
| 전체 App, latency2 | 218.066ms | 249.731ms | 1.172ms |
| 전체 App, AutoNoVsync | 211.731ms | 238.369ms | 0.607ms |
| 전체 App, 생성 후 프로세스 affinity4 | 280.801ms | 343.993ms | 0.966ms |

기본 전체 App의 이전 eframe CPU 보고 중앙값도 약1.97ms다. pinned eframe은 get_current_texture·queue submit·present에 걸린 host 시간을 이 보고에서 제외한다. 따라서 약219ms와 UI 약1ms의 차이를 수식 조판 CPU 비용으로 해석하지 않는다. 남은 주된 비용은 native 렌더링·제출·표시 대기를 포함하는 경로다. 이 실험만으로 그 내부의 각 함수, 순수 GPU 실행, RDP 전송 비용을 따로 확정하지 않는다.

빈 화면은 App의 UI를 건너뛰고 흰 사각형만 그린다. 문서와 mesh workload가 다른 명시적인 대조군이다. 전체 App 실행에서는 보이는 영역 수가17–28로 바뀌었다. 스크롤 중 가시 내용이 변하는 실제 App 경로였으며, frozen idle 그림만 반복한 검사는 아니다. 각 조건은 한 번 실행했으므로 작은 수치 차이를 일반적인 성능 개선으로 판정하지 않는다.

latency2·AutoNoVsync·affinity4는 해결 근거가 없어 제품 설정에 적용하지 않았다. 별도 Glow 비교는 targetable 창을 노출하지 못했으며 성능 표본·성공 대체 경로로 세지 않는다. OpenGL로 단순 교체하면 해결된다는 근거도 없다.

## 측정 제한과 실패 보존

- Computer Use의 화면 캡처는 FrameArrived/window capture timeout으로 실패했다. 창 생성·자체 native 진단은 완료했지만 사람이 보는 화면의 연속 영상·실제 scanout FPS를 확인했다고 주장하지 않는다.
- PresentMon2.6.0 CLI의 own-process 한정 실행은 시작/종료됐지만 CSV 표본을 얻지 못했다. 외부 display/GPU 시간 측정은 미완료다. 사용 방법은 [공식 CLI 문서](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/README-ConsoleApplication.md)를 기준으로 했다.
- 최초 shell launch는 비대화형 세션0에서 Invalid surface로 실패했다. 이 표본을 버벅임 재현으로 세지 않았다. 표의 실행들은 Computer Use로 사용자 세션2에서 별도로 시작했다.
- 진단은 native-profile feature를 명시한 빌드에만 포함된다. 일반 빌드는 sidecar 요청을 읽거나 자동 스크롤하지 않는다. 진단 파일은 절대 경로·새 출력만 허용하고 문서에 쓰지 않는다.

원자료·실행체 복사·각 요청·합성 입력·실험 요약은 로컬 `artifacts/rea/windows-present-20261023/`에 보존한다.

## 진단 재실행

`cargo build --release -p mathcraft-app --features native-profile`로 명시적인 진단 실행체를 만든다. 실행체를 새 전용 폴더에 복사하고, 그 폴더에 `native-profile-request.json`을 둔다. 예시의 scene은 존재하는 문서, output은 아직 없는 절대 경로로 바꾼다.

```json
{
  "scene": "C:/work/example.mcdx",
  "output": "C:/work/profile-new/observation.json",
  "blank": false,
  "glow": false,
  "latency": null,
  "immediate": false
}
```

Windows 사용자 데스크톱 세션에서 그 실행체를 시작한다. 자체112프레임 이후 보고서를 쓰고 종료한다. blank=true는 빈 화면 대조군, latency=2는 프레임 대기 설정, immediate=true는 AutoNoVsync 요청이다. 같은 실행체를 새 폴더·새 출력으로 복사해 한 조건씩 실행한다. 코드 계측에 합성 입력을 사용하는 진단이며 실제 마우스 이벤트 도착 시간을 측정하는 도구는 아니다.

Windows 기존 정상2180검사·fmt·workspace/backend Clippy·빌드·참조38건이 통과했다. native-profile feature는 Windows와 peets 양쪽에서 Clippy `-D warnings`를 통과했다. 일반 배포 빌드에서는 이 feature를 지정하지 않는다.

## 수정 방향과 현재 판정

**Windows 전체 스크롤 지연은 아직 해결되지 않았다.** 하드웨어 어댑터가 있는 환경과 이와 같은 software/원격 환경을 구분하고, 후자에서 유효한2D 렌더링 대체 경로를 같은 native 문서·색상·글꼴·입력 계약으로 검증해야 한다. 소프트웨어 어댑터를 무조건 거부하거나 무효였던 VSync 변경을 배포하는 것은 수정이 아니다. 프레임 조판 캐시 추가나 UI 디자인 개편만으로 이번 병목의 완료를 선언하지 않는다.

Kubuntu가 부드럽다는 사용자 확인은 OS 경로 비교 근거다. 이번 Windows native 수치를 Kubuntu 실측 수치로 대체하지 않는다.
