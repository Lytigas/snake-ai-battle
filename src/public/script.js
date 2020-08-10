const canvas = document.getElementById("gc");
const ctx = canvas.getContext("2d");

const GRIDE_SIZE = 20;

function render(data) {
  canvas.width = GRIDE_SIZE * data.width + 2;
  canvas.height = GRIDE_SIZE * data.height + 2;

  ctx.fillStyle = "black";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  for (let x = 0; x < data.width; x++) {
    for (let y = 0; y < data.height; y++) {
      let idx = y * data.width + x;
      let square = data.data[idx];
      if (square == "Red") {
        ctx.fillStyle = "red";
      } else if (square == "Blue") {
        ctx.fillStyle = "blue";
      } else {
        ctx.fillStyle = "grey";
      }
      ctx.fillRect(
        x * GRIDE_SIZE + 2,
        y * GRIDE_SIZE + 2,
        GRIDE_SIZE - 2,
        GRIDE_SIZE - 2
      );
    }
  }
}

const sse = new EventSource("watch");
sse.addEventListener("render", (e) => {
  let data = JSON.parse(e.data);
  console.log(data);
  render(data);
});
