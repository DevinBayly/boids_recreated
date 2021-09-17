#version 450

const vec2 positions[3] = vec2[3] (
  vec2(0,0),
  vec2(.5,0),
  vec2(0,1)
);



void main ()
{
  gl_PointSize = 2.0;
  gl_Position = vec4(positions[gl_VertexIndex], 1.0, 1.0);
}