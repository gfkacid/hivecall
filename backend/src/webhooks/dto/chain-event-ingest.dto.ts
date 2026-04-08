import { IsObject, IsString, MinLength } from 'class-validator';

export class ChainEventIngestDto {
  @IsString()
  @MinLength(1)
  chain: string;

  @IsString()
  @MinLength(1)
  dedupeKey: string;

  @IsString()
  @MinLength(1)
  source: string;

  @IsObject()
  payload: Record<string, unknown>;
}
