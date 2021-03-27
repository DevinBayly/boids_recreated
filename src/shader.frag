#version 450


// LOOK: corresponds with outColor from particle.vert
layout (location = 0) in vec2 inColor;
layout (location = 1) in flat int outVid;
layout (location = 2) in flat int outTid;

layout (location = 0) out vec4 outFragColor;

layout (set = 0, binding = 0,rgba32f) uniform image2D t_view;
// dimensions of image are 256x256 so use the outVID to read and write from texture
void main ()
{
	outFragColor.a = 1.0;
	outFragColor.rgb = vec3(0.0);
	// see if the value in the texture is already been updated
	//outFragColor.rgb = vec3(inColor.x, abs(inColor.y), -inColor.x) * 10.0;

	float x = mod(outTid,256);
	float y = floor(outTid/256);
	// do I know for sure what the actual number range is here? is it 0-1?
	//imageStore(t_view,ivec2(x,y),vec4(1.0));
	float texture_value = imageLoad(t_view,ivec2(x,y)).r;
	if (texture_value > .7) {
		outFragColor.r = 1*inColor.x;
		imageStore(t_view,ivec2(x,y),vec4(texture_value - .01));
	} else if (texture_value > .5) {
	// update texture at this point
		// use the texture  value to color our square
		outFragColor.b = 1*inColor.x ;
		imageStore(t_view,ivec2(x,y),vec4(texture_value -.01));
	} else if (texture_value > .3) {
		outFragColor.g = 1*inColor.x ;
		imageStore(t_view,ivec2(x,y),vec4(texture_value -.01));
	} else {
		imageStore(t_view,ivec2(x,y),vec4(1.0));
		outFragColor.r = 1*inColor.x;
	}
}