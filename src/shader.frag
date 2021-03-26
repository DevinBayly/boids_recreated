#version 450


// LOOK: corresponds with outColor from particle.vert
layout (location = 0) in vec2 inColor;

layout (location = 0) out vec4 outFragColor;

layout (rgba8,set = 0, binding = 0) uniform image2D t_view;

void main ()
{
	outFragColor.a = 1.0;
	// see if the value in the texture is already been updated
	//outFragColor.rgb = vec3(inColor.x, abs(inColor.y), -inColor.x) * 10.0;
	float texture_value = imageLoad(t_view,ivec2(0,0)).r;
	if (texture_value.x == .1) {
		outFragColor.r = 1.0;
		imageStore(t_view,ivec2(0,0),vec4(.2));
	} else {
		outFragColor.g = 1.0;
		imageStore(t_view,ivec2(0,0),vec4(.1));
	}
}