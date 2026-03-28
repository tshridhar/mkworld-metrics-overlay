use ocrs::{OcrEngine, ImageSource};
use rten_tensor::{NdTensor, AsView};
fn main() {
    let tensor = NdTensor::from_data([3, 10, 10], vec![0f32; 300]);
    let img_src = ImageSource::from_bytes(b"temp", (10, 10));
    let img_src2 = ImageSource::from_tensor(tensor.view());
}
