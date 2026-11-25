//! 프로필 이미지 규칙 테스트. 임시 SQLite 파일과 임시 업로드 폴더로 서비스를 만든다.
#![cfg(feature = "server")]

use api::service::OnairService;

async fn open(dir: &std::path::Path) -> OnairService {
    OnairService::open(dir.join("onair.sqlite"), dir.join("uploads"))
        .await
        .unwrap()
}

/// 매직 바이트만 맞춘 가짜 이미지.
fn png() -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    b.extend_from_slice(b"png-body");
    b
}

fn jpeg() -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0];
    b.extend_from_slice(b"jpeg-body");
    b
}

fn webp() -> Vec<u8> {
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&[0x10, 0, 0, 0]);
    b.extend_from_slice(b"WEBPVP8 ");
    b
}

fn files_in(service: &OnairService) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(service.upload_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect()
}

#[tokio::test]
async fn 프로필_이미지를_올리면_업로드_폴더에_저장되고_채널에_연결된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let bytes = png();

    let channel = service
        .upload_profile_image("test-key-1", &bytes)
        .await
        .unwrap();

    let url = channel.profile_image.clone().unwrap();
    let file_name = url.strip_prefix("/uploads/").unwrap();
    assert!(file_name.ends_with(".png"));
    assert_eq!(
        std::fs::read(service.upload_dir().join(file_name)).unwrap(),
        bytes
    );
    let listed = service.list_channels().await.unwrap();
    assert_eq!(listed[0].profile_image, Some(url.clone()));
    assert_eq!(
        service.get_channel(channel.id).await.unwrap().profile_image,
        Some(url)
    );
    assert!(listed[1].profile_image.is_none());
}

#[tokio::test]
async fn jpg와_webp도_받고_확장자는_내용으로_정한다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let j = service
        .upload_profile_image("test-key-1", &jpeg())
        .await
        .unwrap();
    let w = service
        .upload_profile_image("test-key-2", &webp())
        .await
        .unwrap();

    assert!(j.profile_image.unwrap().ends_with(".jpg"));
    assert!(w.profile_image.unwrap().ends_with(".webp"));
}

#[tokio::test]
async fn 이미지가_아닌_내용은_이유와_함께_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    // 파일 이름은 서비스가 받지도 않는다. 내용만 본다.
    for bytes in [
        &b"GIF89a....."[..],
        b"<html>hi</html>",
        b"",
        b"RIFF\0\0\0\0WAVEfmt ",
    ] {
        let err = service
            .upload_profile_image("test-key-1", bytes)
            .await
            .unwrap_err();
        assert_eq!(err, "jpg, png, webp 이미지만 올릴 수 있습니다");
    }
    assert!(files_in(&service).is_empty());
    assert!(service.get_channel(1).await.unwrap().profile_image.is_none());
}

#[tokio::test]
async fn 이미지가_2mb를_넘으면_이유와_함께_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let mut ok = png();
    ok.resize(2 * 1024 * 1024, 0);
    let mut big = png();
    big.resize(2 * 1024 * 1024 + 1, 0);

    service
        .upload_profile_image("test-key-1", &ok)
        .await
        .unwrap();
    let err = service
        .upload_profile_image("test-key-1", &big)
        .await
        .unwrap_err();

    assert_eq!(err, "이미지는 2MB 이하여야 합니다");
}

#[tokio::test]
async fn 새_이미지를_올리면_교체되고_이전_파일은_남지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let first = service
        .upload_profile_image("test-key-1", &png())
        .await
        .unwrap();

    let second = service
        .upload_profile_image("test-key-1", &jpeg())
        .await
        .unwrap();

    assert_ne!(first.profile_image, second.profile_image);
    let files = files_in(&service);
    assert_eq!(files.len(), 1);
    let name = second.profile_image.unwrap();
    assert!(files[0].ends_with(name.strip_prefix("/uploads/").unwrap()));
}

#[tokio::test]
async fn 틀린_송출_키로는_업로드가_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let err = service
        .upload_profile_image("nope", &png())
        .await
        .unwrap_err();

    assert_eq!(err, "송출 키가 올바르지 않습니다");
    assert!(files_in(&service).is_empty());
}

#[tokio::test]
async fn 저장된_파일_이름에는_송출_키가_드러나지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;

    let channel = service
        .upload_profile_image("test-key-1", &png())
        .await
        .unwrap();

    assert!(!channel.profile_image.unwrap().contains("test-key-1"));
}
