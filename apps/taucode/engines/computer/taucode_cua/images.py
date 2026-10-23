"""Bound vision image cost without changing aspect ratio."""
from io import BytesIO
from PIL import Image


def downscale(png):
    with Image.open(BytesIO(png)) as image:
        if max(image.size) <= 1920:
            return png, image.size
        image.thumbnail((1920, 1920), Image.Resampling.LANCZOS)
        output = BytesIO()
        image.save(output, format='PNG')
        return output.getvalue(), image.size
