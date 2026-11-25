//! onair 서비스 테스트. 테스트마다 비어 있는 임시 SQLite 파일과 임시 업로드 폴더로 서비스를 만든다.
#![cfg(feature = "server")]

use api::service::OnairService;

async fn open(dir: &std::path::Path) -> OnairService {
    OnairService::open(dir.join("onair.sqlite"), dir.join("uploads"))
        .await
        .unwrap()
}

#[tokio::test]
async fn 처음_켜면_테스트_채널_3개가_들어간다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let names: Vec<String> = service
        .list_channels()
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();

    assert_eq!(names, ["테스트 채널 1", "테스트 채널 2", "테스트 채널 3"]);
}

#[tokio::test]
async fn 다시_켜도_시드가_중복되지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    drop(open(dir.path()).await);

    let service = open(dir.path()).await;

    assert_eq!(service.list_channels().await.unwrap().len(), 3);
}

#[tokio::test]
async fn sqlite_파일을_지우고_다시_켜면_시드_상태로_돌아간다() {
    let dir = tempfile::tempdir().unwrap();
    let first = open(dir.path()).await;
    let before = first.list_channels().await.unwrap();
    first.close().await;
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            std::fs::remove_file(path).unwrap();
        }
    }

    let after = open(dir.path()).await.list_channels().await.unwrap();

    assert_eq!(before, after);
}

#[tokio::test]
async fn 프로필_이미지가_없는_채널은_이미지_없음으로_나온다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let channels = service.list_channels().await.unwrap();

    assert!(channels.iter().all(|c| c.profile_image.is_none()));
}

#[tokio::test]
async fn 채널_아이디로_채널_하나를_조회한다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let second = service.list_channels().await.unwrap().remove(1);

    let channel = service.get_channel(second.id).await.unwrap();

    assert_eq!(channel.id, second.id);
    assert_eq!(channel.name, "테스트 채널 2");
}

#[tokio::test]
async fn 없는_채널을_조회하면_이유가_담긴_실패다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let err = service.get_channel(9999).await.unwrap_err();

    assert_eq!(err, "없는 채널입니다");
}

#[tokio::test]
async fn 시청자_쪽_응답에_송출_키가_담기지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let list = service.list_channels().await.unwrap();
    let one = service.get_channel(list[0].id).await.unwrap();

    let json = format!(
        "{}{}",
        serde_json::to_string(&list).unwrap(),
        serde_json::to_string(&one).unwrap()
    );

    assert!(!json.contains("key"), "응답에 송출 키가 있다: {json}");
    for key in api::service::SEED_STREAM_KEYS {
        assert!(!json.contains(key));
    }
}

#[tokio::test]
async fn 채널을_만들면_송출_키가_나오고_목록에_나타난다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let created = service.create_channel("내 채널").await.unwrap();

    assert_eq!(created.name, "내 채널");
    assert!(!created.stream_key.is_empty());
    let names: Vec<String> = service
        .list_channels()
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names.last().unwrap(), "내 채널");
    assert_eq!(names.len(), 4);
}

#[tokio::test]
async fn 송출_키는_고정_접두어가_붙은_무작위_문자열이다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let a = service.create_channel("가").await.unwrap().stream_key;
    let b = service.create_channel("나").await.unwrap().stream_key;

    assert!(a.starts_with("onair_") && b.starts_with("onair_"));
    assert_ne!(a, b);
    assert!(a.len() >= "onair_".len() + 32, "너무 짧다: {a}");
    assert!(
        a.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "URL에 그대로 쓸 수 없는 문자: {a}"
    );
}

#[tokio::test]
async fn 채널_이름은_앞뒤_공백을_자르고_저장한다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let created = service.create_channel("  공백 채널  ").await.unwrap();

    assert_eq!(created.name, "공백 채널");
}

#[tokio::test]
async fn 빈_이름의_채널은_이유와_함께_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let err = service.create_channel("   ").await.unwrap_err();

    assert_eq!(err, "채널 이름을 입력하세요");
    assert_eq!(service.list_channels().await.unwrap().len(), 3);
}

#[tokio::test]
async fn 이미_있는_이름의_채널은_이유와_함께_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let err = service.create_channel(" 테스트 채널 1 ").await.unwrap_err();

    assert_eq!(err, "이미 있는 채널 이름입니다");
    assert_eq!(service.list_channels().await.unwrap().len(), 3);
}

#[tokio::test]
async fn 송출_키로_스튜디오에_들어가면_그_채널이_나온다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let created = service.create_channel("스튜디오 채널").await.unwrap();

    let studio = service.enter_studio(&created.stream_key).await.unwrap();

    assert_eq!(studio, created);
    let seed = service.enter_studio("test-key-2").await.unwrap();
    assert_eq!(seed.name, "테스트 채널 2");
    assert_eq!(seed.stream_key, "test-key-2");
}

#[tokio::test]
async fn 틀린_송출_키는_스튜디오에서_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let err = service.enter_studio("nope").await.unwrap_err();

    assert_eq!(err, "송출 키가 올바르지 않습니다");
    assert!(service.enter_studio("").await.is_err());
}
