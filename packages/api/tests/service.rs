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

    assert!(channels.iter().all(|c| !c.has_profile_image));
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
