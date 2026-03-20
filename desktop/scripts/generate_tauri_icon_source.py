from pathlib import Path
import os

def main() -> None:
    try:
        from PIL import Image
    except ImportError as exc:
        raise SystemExit(
            "Pillow is required to generate Tauri icons. Install python3-pil and retry."
        ) from exc

    desktop_dir = Path(__file__).resolve().parents[1]
    repo_root = Path(
        os.environ.get("NOMADTERM_REPO_ROOT", str(Path(__file__).resolve().parents[3]))
    )
    source = repo_root / "assets" / "logo.png"
    output = desktop_dir / "src-tauri" / "icons" / "logo-square.png"

    if not source.exists():
        raise SystemExit(f"Shared icon source not found: {source}")

    canvas_size = 256

    with Image.open(source) as original:
        image = original.convert("RGBA")
        scale = min(canvas_size / image.width, canvas_size / image.height)
        resized = image.resize(
            (round(image.width * scale), round(image.height * scale)),
            Image.Resampling.LANCZOS,
        )

    canvas = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    offset = (
        (canvas_size - resized.width) // 2,
        (canvas_size - resized.height) // 2,
    )
    canvas.paste(resized, offset, resized)
    canvas.save(output)


if __name__ == "__main__":
    main()
