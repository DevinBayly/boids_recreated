#version 450

layout (location = 0) in vec2 vpos; // this is a vertex value 
layout (location = 1 ) in vec3 gasPos;

void main ()
{
  //gl_PointSize = 2.0;
  gl_Position = vec4(vpos + (gasPos.xy/100.0)*2.0 - 1.0, 1.0, 1.0);
}