#version 450


// LOOK: corresponds with outColor from particle.vert
layout (location = 0) in vec2 inColor;

layout (location = 0) out vec4 outFragColor;

void main ()
{
	outFragColor.a = 1.0;
	outFragColor.rgb = vec3(inColor.x, abs(inColor.y), -inColor.x) * 10.0;
}